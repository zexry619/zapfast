//! The worker's side of a 1:1 call: commands, signaling routing, and media events.
//!
//! One call at a time. [`Worker::call`] holds the only one, and every entry point here refuses to
//! start or accept a second while it exists — a double click, a second chat's call button, and a
//! second inbound offer all land on the same refusal.
//!
//! The state the UI renders is never computed here: it comes from the backend. `<accept>` moves an
//! outgoing call out of dialing, the media plane's `RelayAllocated` is what makes a call Active and
//! starts the timer, and a `<reject>`/`<terminate>`/relay loss ends it with the reason the peer
//! gave. Nothing is inferred from the button that was pressed.

use std::time::{Duration, Instant};

use super::*;
use crate::calls::{self, Call, CallUpdate, VideoTick};
use whatsapp_rust::types::call::IncomingCall;
use whatsapp_rust::voip::{CallEvent, KeyframeUrgency};

/// The call this worker owns: its state, and the channels the tasks around it feed.
///
/// Dropped the moment the call reaches a terminal phase, which is what releases the media tasks,
/// the microphone and speaker streams and the camera thread.
pub(super) struct CallRuntime {
    pub(super) call: Call,
    /// Media-plane events, and the signal that the media task finished.
    pub(super) events: async_channel::Receiver<CallRuntimeEvent>,
    /// Video frames for the UI.
    pub(super) frames: Option<async_channel::Receiver<VideoTick>>,
    /// Whether the media-plane watcher was started for this call's handle.
    watching: bool,
    /// The snapshot the UI was last handed, so the periodic check publishes only real changes.
    last: CallUpdate,
    local_ticks: u32,
    remote_ticks: u32,
    tick_timer: Instant,
    cur_video_bitrate: u32,
    min_video_bitrate: u32,
    max_video_bitrate: u32,
    last_bitrate_adjust: Instant,
    last_probe_up: Instant,
    last_keyframe_req: Instant,
    last_peer_keyframe_req: Instant,
}

/// What a call's background tasks report back to the worker loop.
pub(super) enum CallRuntimeEvent {
    /// One event off the call's media plane.
    Media(CallEvent),
    /// The media task is gone: relay disconnect, send failure, or hangup.
    Ended,
}

impl CallRuntime {
    /// A runtime whose media plane is not watched yet: a call that is still ringing has no handle
    /// to watch, and gets one the moment it is placed or answered.
    pub(super) fn new(call: Call, frames: Option<async_channel::Receiver<VideoTick>>) -> Self {
        let (_tx, events) = async_channel::bounded(64);
        let last = call.update();
        let (base_bitrate, min_bitrate, max_bitrate) = (250_000, 140_000, 450_000);
        Self {
            call,
            events,
            frames,
            watching: false,
            last,
            local_ticks: 0,
            remote_ticks: 0,
            tick_timer: Instant::now(),
            cur_video_bitrate: base_bitrate,
            min_video_bitrate: min_bitrate,
            max_video_bitrate: max_bitrate,
            last_bitrate_adjust: Instant::now(),
            last_probe_up: Instant::now(),
            last_keyframe_req: Instant::now() - Duration::from_secs(5),
            last_peer_keyframe_req: Instant::now() - Duration::from_secs(5),
        }
    }

    /// Starts forwarding this call's media events and its media-finished signal, once.
    pub(super) fn watch(&mut self) {
        if self.watching {
            return;
        }
        let Some(handle) = self.call.handle() else {
            return;
        };
        self.watching = true;
        let (tx, events) = async_channel::bounded(64);
        self.events = events;
        tokio::spawn(async move {
            let Some(mut media) = handle.take_events() else {
                let _ = tx.send(CallRuntimeEvent::Ended).await;
                return;
            };
            loop {
                tokio::select! {
                    event = media.recv() => match event {
                        Ok(event) => {
                            if tx.send(CallRuntimeEvent::Media(event)).await.is_err() {
                                return;
                            }
                        }
                        Err(_) => {
                            let _ = tx.send(CallRuntimeEvent::Ended).await;
                            return;
                        }
                    },
                    _ = handle.wait_ended() => {
                        let _ = tx.send(CallRuntimeEvent::Ended).await;
                        return;
                    }
                }
            }
        });
    }
}

impl Worker {
    // -----------------------------------------------------------------------
    // Command entry points
    // -----------------------------------------------------------------------

    /// Hands the call screen the microphones, speakers and cameras it can offer, and keeps them:
    /// they are how a device that goes away is named by the description the user saw.
    pub(super) async fn emit_call_devices(&mut self) -> calls::DeviceList {
        let devices = discover_devices().await;
        self.publish_call_devices(devices.clone());
        devices
    }

    /// Stores and publishes a device list, so the pickers and the fallback naming share one
    /// snapshot of the machine.
    pub(super) fn publish_call_devices(&mut self, devices: calls::DeviceList) {
        self.call_devices = devices.clone();
        self.emit(Event::CallDevices(Box::new(devices)));
    }

    /// Re-sends the current call after privacy recovery, since call updates are withheld while the
    /// lock state is unknown.
    ///
    /// Nothing is rediscovered: the last published list is what the picker already had, and the
    /// runtime's own snapshot is what the surface should draw. The chat can finally be judged
    /// private or not, so a call that arrived mid-recovery rings behind the right lock state rather
    /// than being dropped for the whole recovery.
    pub(super) fn replay_call(&mut self) {
        let Some(runtime) = self.call.as_ref() else {
            return;
        };
        let devices = self.call_devices.clone();
        let update = runtime.last.clone();
        self.publish_call_devices(devices);
        self.emit_call(update);
    }

    /// Publishes one call state, and lets the call go once it reaches a terminal phase.
    ///
    /// The UI keeps rendering the snapshot it was handed; what is released here is the media, so a
    /// finished call holds no child process and no task.
    pub(super) fn emit_call(&mut self, update: CallUpdate) {
        let finished = !update.phase.is_live();
        // Remembered so the periodic check can tell a real change from a snapshot it already sent.
        if let Some(runtime) = self.call.as_mut()
            && runtime.call.generation() == update.generation
        {
            runtime.last = update.clone();
        }
        self.emit(Event::Call(Box::new(update.clone())));
        if finished {
            let duration = update
                .started
                .map(|s| s.elapsed().as_secs() as i64)
                .unwrap_or(0);
            let status = match update.outcome {
                Some(crate::calls::CallOutcome::Answered) => CallLogStatus::Connected,
                Some(
                    crate::calls::CallOutcome::Declined
                    | crate::calls::CallOutcome::DeclinedElsewhere,
                ) => CallLogStatus::Rejected,
                Some(crate::calls::CallOutcome::Missed | crate::calls::CallOutcome::NoAnswer) => {
                    CallLogStatus::Missed
                }
                Some(crate::calls::CallOutcome::Failed | crate::calls::CallOutcome::Busy) => {
                    CallLogStatus::Failed
                }
                _ => {
                    if duration > 0 {
                        CallLogStatus::Connected
                    } else {
                        CallLogStatus::Other
                    }
                }
            };
            let call_id = format!("call-{}-{}", update.chat, crate::util::now());
            let entry = CallLogEntry {
                call_id,
                peer: update.chat.clone(),
                peer_name: None,
                from_me: matches!(update.direction, crate::model::CallDirection::Outgoing),
                timestamp: crate::util::now(),
                duration,
                is_video: update.video,
                status,
            };
            let _ = self.archive.insert_call_log(&entry);
            self.emit_call_logs();
            self.call = None;
        }
    }

    /// Whether a call is already up, which every entry point refuses to double.
    pub(super) fn call_busy(&mut self) -> bool {
        if self.call.is_some() {
            log::warn!("[CALL] refusing a new call: one is already up");
            return true;
        }
        false
    }

    pub(super) async fn start_call(&mut self, chat: ChatId, video: bool) {
        if self.call_busy() {
            return;
        }
        if !callable_chat(&chat) {
            log::warn!("[CALL] refusing a call to a chat that is not one to one");
            self.emit(Event::Error("Calls are one to one only".to_owned()));
            return;
        }
        let Some(client) = self.client.clone() else {
            log::warn!("[CALL] cannot start a call: WhatsApp is not connected");
            self.emit(Event::Error(
                "ZapFast is not connected to WhatsApp".to_owned(),
            ));
            return;
        };
        // The devices the settings remember, checked against the machine first: a headset that was
        // switched off since the last call falls back to the system default and says so, instead of
        // opening a stream that can never deliver a frame.
        let devices = self.emit_call_devices().await;
        let wanted = self.call_defaults.clone();
        let resolved =
            calls::resolve_devices(&devices, wanted.microphone, wanted.speaker, wanted.camera);
        match Call::place(
            &client,
            chat.to_string(),
            video,
            resolved.microphone,
            resolved.speaker,
            resolved.camera,
        )
        .await
        {
            Ok((mut call, frames)) => {
                call.set_lost_devices(resolved.lost_devices);
                let mut runtime = CallRuntime::new(call, frames);
                runtime.watch();
                let update = runtime.call.update();
                self.call = Some(runtime);
                self.emit_call(update);
            }
            Err(error) => {
                log::error!("[CALL] could not start the call: {error}");
                self.emit(Event::Error(format!(
                    "The call could not be started: {error}"
                )));
            }
        }
    }

    pub(super) async fn answer_call(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let update = match self.call.as_mut() {
            Some(runtime) => {
                // Whatever the user picked while the phone was ringing is what the media binds to,
                // checked against the machine so one that vanished falls back rather than opening a
                // stream that can never deliver.
                let (microphone, speaker, camera) = runtime.call.selections();
                let resolved =
                    calls::resolve_devices(&discover_devices().await, microphone, speaker, camera);
                let answered = runtime
                    .call
                    .answer(
                        &client,
                        resolved.microphone,
                        resolved.speaker,
                        resolved.camera,
                    )
                    .await;
                match answered {
                    Ok(frames) => {
                        if frames.is_some() {
                            runtime.frames = frames;
                        }
                        runtime.call.set_lost_devices(resolved.lost_devices);
                        runtime.watch();
                        Some(runtime.call.update())
                    }
                    Err(error) => {
                        log::error!("[CALL] could not answer the call: {error}");
                        self.emit(Event::Error(format!(
                            "The call could not be answered: {error}"
                        )));
                        return;
                    }
                }
            }
            None => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) async fn decline_call(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let update = match self.call.as_mut() {
            Some(runtime) => match runtime.call.decline(&client).await {
                Ok(()) => Some(runtime.call.update()),
                Err(error) => {
                    log::error!("[CALL] could not decline the call: {error}");
                    return;
                }
            },
            None => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) async fn hangup_call(&mut self) {
        let client = self.client.clone();
        let update = match self.call.as_mut() {
            Some(runtime) => Some(runtime.call.hangup(client.as_ref()).await),
            None => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) async fn set_call_muted(&mut self, muted: bool) {
        let update = match self.call.as_mut() {
            Some(runtime) => runtime.call.set_muted(muted).await,
            None => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) async fn set_call_camera(&mut self, on: bool) {
        let outcome = match self.call.as_mut() {
            Some(runtime) => {
                if on && !runtime.call.is_video() {
                    // A voice call: ask the peer for video and start draining the frames it brings,
                    // from the camera the settings remember.
                    let camera = self.call_defaults.camera.clone();
                    match runtime.call.upgrade_to_video(camera).await {
                        Ok(frames) => {
                            if frames.is_some() {
                                runtime.frames = frames;
                            }
                            Ok(runtime.call.update())
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    runtime.call.set_camera(on).await
                }
            }
            None => return,
        };
        match outcome {
            Ok(update) => self.emit_call(update),
            Err(error) => {
                log::error!("[CALL] the camera could not be toggled: {error}");
                self.emit(Event::Error(error.to_string()));
            }
        }
    }

    pub(super) async fn set_call_screen_share(&mut self, on: bool) {
        let outcome = match self.call.as_mut() {
            Some(runtime) => runtime.call.set_screen_share(on).await,
            None => return,
        };
        match outcome {
            Ok(update) => self.emit_call(update),
            Err(error) => {
                log::error!("[CALL] screen share could not be toggled: {error}");
                self.emit(Event::Error(error.to_string()));
            }
        }
    }

    pub(super) fn set_call_microphone(&mut self, device: Option<String>) {
        // The picker is also the preference: the next call opens where this one was left.
        self.call_defaults.microphone = device.clone();
        let update = self
            .call
            .as_mut()
            .map(|runtime| runtime.call.set_microphone(device));
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) fn set_call_speaker(&mut self, device: Option<String>) {
        self.call_defaults.speaker = device.clone();
        let update = self
            .call
            .as_mut()
            .map(|runtime| runtime.call.set_speaker(device));
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    pub(super) fn set_call_camera_device(&mut self, device: Option<String>) {
        self.call_defaults.camera = device.clone();
        let outcome = self
            .call
            .as_mut()
            .map(|runtime| runtime.call.set_camera_device(device));
        match outcome {
            Some(Ok(update)) => self.emit_call(update),
            Some(Err(error)) => {
                log::error!("[CALL] the camera could not be switched: {error}");
                self.emit(Event::Error(error.to_string()));
            }
            None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Backend events
    // -----------------------------------------------------------------------

    /// One event from the current call's media plane.
    pub(super) fn call_runtime(&mut self, event: CallRuntimeEvent) {
        let update = match self.call.as_mut() {
            Some(runtime) => match event {
                CallRuntimeEvent::Ended => runtime.call.media_ended(),
                CallRuntimeEvent::Media(media) => {
                    if matches!(media, CallEvent::RelayAllocated) && runtime.call.is_video() {
                        if let Some(handle) = runtime.call.handle() {
                            handle.request_peer_keyframe(KeyframeUrgency::Immediate);
                        }
                        runtime.call.request_local_keyframe();
                    }
                    let mut needs_keyframe = matches!(media, CallEvent::VideoKeyframeNeeded);
                    if let CallEvent::RtcpReceived {
                        report_blocks,
                        feedback,
                        ..
                    } = &media
                    {
                        let now = Instant::now();
                        let max_loss = report_blocks
                            .iter()
                            .map(|b| b.fraction_lost)
                            .max()
                            .unwrap_or(0);
                        let has_pli = feedback
                            .iter()
                            .any(|f| f.packet_type == 206 && matches!(f.fmt, 1 | 4));
                        if max_loss > 0 {
                            for block in report_blocks {
                                if block.fraction_lost > 0 {
                                    let pct = (block.fraction_lost as f32 / 255.0) * 100.0;
                                    log::warn!(
                                        "[CALL][RTCP] peer packet loss: {}/255 ({:.1}%), cum_lost={}, jitter={}, seq={}",
                                        block.fraction_lost,
                                        pct,
                                        block.cumulative_lost,
                                        block.jitter,
                                        block.extended_highest_sequence
                                    );
                                }
                            }
                        }
                        if has_pli {
                            needs_keyframe = true;
                            log::info!("[CALL][RTCP] peer requested keyframe via RTCP PLI/FIR");
                        }
                        if max_loss >= 25 {
                            if runtime.last_bitrate_adjust.elapsed() >= Duration::from_millis(1500)
                            {
                                runtime.last_bitrate_adjust = now;
                                runtime.last_probe_up = now;
                                let new_rate = (runtime.cur_video_bitrate * 85 / 100)
                                    .max(runtime.min_video_bitrate);
                                if new_rate < runtime.cur_video_bitrate {
                                    runtime.cur_video_bitrate = new_rate;
                                    runtime.call.set_video_target_bitrate(new_rate);
                                    log::info!(
                                        "[CALL][BWE] RTCP packet loss ({}/255): backed off target bitrate to {}kbps",
                                        max_loss,
                                        new_rate / 1000
                                    );
                                }
                            }
                        } else if max_loss == 0
                            && runtime.last_probe_up.elapsed() >= Duration::from_secs(4)
                        {
                            runtime.last_probe_up = now;
                            if runtime.cur_video_bitrate < runtime.max_video_bitrate {
                                let new_rate = (runtime.cur_video_bitrate + 25_000)
                                    .min(runtime.max_video_bitrate);
                                runtime.cur_video_bitrate = new_rate;
                                runtime.call.set_video_target_bitrate(new_rate);
                                log::info!(
                                    "[CALL][BWE] connection clean (0 loss): probed target bitrate to {}kbps",
                                    new_rate / 1000
                                );
                            }
                        }
                    }
                    if needs_keyframe {
                        let now = Instant::now();
                        if runtime.last_keyframe_req.elapsed() >= Duration::from_millis(1500) {
                            runtime.last_keyframe_req = now;
                            log::info!(
                                "[CALL] peer requested keyframe: forcing local IDR keyframe"
                            );
                            runtime.call.request_local_keyframe();
                        } else {
                            log::debug!(
                                "[CALL] peer requested keyframe (throttled): skipping duplicate within 1500ms"
                            );
                        }
                    }
                    runtime.call.media(&media)
                }
            },
            None => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    /// One video frame from the current call, straight to the UI.
    pub(super) fn call_frame(&mut self, tick: VideoTick) {
        if matches!(tick, VideoTick::NeedsKeyframe) {
            if let Some(runtime) = self.call.as_mut()
                && let Some(handle) = runtime.call.handle()
            {
                if runtime.last_peer_keyframe_req.elapsed() >= Duration::from_millis(1200) {
                    runtime.last_peer_keyframe_req = Instant::now();
                    log::info!("[CALL] requesting peer IDR keyframe via RTCP PLI");
                    handle.request_peer_keyframe(KeyframeUrgency::Immediate);
                }
            }
            return;
        }

        if let Some(runtime) = self.call.as_mut() {
            match &tick {
                VideoTick::Local(_) => runtime.local_ticks += 1,
                VideoTick::Remote(_) => runtime.remote_ticks += 1,
                VideoTick::NeedsKeyframe => {}
            }
            if runtime.tick_timer.elapsed() >= std::time::Duration::from_secs(1) {
                let dur = runtime.tick_timer.elapsed().as_secs_f32();
                let remote_fps = runtime.remote_ticks as f32 / dur;
                let local_fps = runtime.local_ticks as f32 / dur;
                log::info!(
                    "[CALL][DISPATCH] to_ui: remote_fps={remote_fps:.1} local_fps={local_fps:.1}"
                );
                runtime.tick_timer = std::time::Instant::now();
                runtime.local_ticks = 0;
                runtime.remote_ticks = 0;
            }
        }
        match tick {
            VideoTick::Local(image) => {
                self.emit(Event::CallVideo {
                    local: Some(image),
                    remote: None,
                });
            }
            VideoTick::Remote(image) => {
                let first = self
                    .call
                    .as_mut()
                    .is_some_and(|runtime| runtime.call.saw_remote_video());
                self.emit(Event::CallVideo {
                    local: None,
                    remote: Some(image),
                });
                if first && let Some(runtime) = self.call.as_ref() {
                    let update = runtime.call.update();
                    self.emit(Event::Call(Box::new(update)));
                }
            }
            VideoTick::NeedsKeyframe => {}
        }
    }

    /// Routes one inbound `<call>` stanza: either it belongs to the call we own, or it is a new
    /// offer that should ring.
    ///
    /// This is where an outgoing call learns it was answered, rejected, or ended, so the phase the
    /// UI shows is the peer's, not the dialer's.
    pub(super) async fn call_signaling(&mut self, incoming: &IncomingCall) {
        let action = &incoming.action;
        let mine = self
            .call
            .as_ref()
            .is_some_and(|runtime| runtime.call.call_id() == action.call_id());
        if mine {
            let update = self
                .call
                .as_mut()
                .and_then(|runtime| runtime.call.signaling(action));
            if let Some(update) = update {
                self.emit_call(update);
            }
            return;
        }
        if !calls::is_offer(action) {
            return;
        }
        if self.call_busy() {
            return;
        }
        let chat = self.canonical(&incoming.from);
        // A group, broadcast or newsletter offer never reaches the 1:1 call surface. The interface
        // hides the buttons for those chats, but the worker is the boundary: a non-direct offer is
        // refused here rather than rung, so no call is created and no history entry is written.
        if !callable_chat(&chat) {
            log::warn!("[CALL] refusing an incoming offer from a chat that is not one to one");
            return;
        }
        let video = calls::offer_is_video(action);
        // The remembered devices are pre-selected on the prompt, so answering picks up right where
        // the last call left off.
        let call = Call::ringing(
            chat,
            Box::new(incoming.clone()),
            video,
            self.call_defaults.clone(),
        );
        let update = call.update();
        self.call = Some(CallRuntime::new(call, None));
        self.emit_call_devices().await;
        self.emit_call(update);
    }

    /// A call we were ringing or talking on was resolved on another of the account's devices, or
    /// the caller gave up before anyone answered.
    pub(super) fn call_resolved(&mut self, call_id: &str) {
        let update = match self.call.as_mut() {
            Some(runtime) if runtime.call.call_id() == call_id => runtime.call.resolved_elsewhere(),
            _ => None,
        };
        if let Some(update) = update {
            self.emit_call(update);
        }
    }

    /// Releases the call on the way out, so quitting leaves no child process or task behind.
    pub(super) async fn shutdown_call(&mut self) {
        if let Some(mut runtime) = self.call.take() {
            runtime.call.hangup(self.client.as_ref()).await;
        }
    }

    /// Keeps a live call's picture of the machine honest.
    ///
    /// A device the user picked can disappear mid-call — a Bluetooth headset switching off, a camera
    /// unplugged, a sound card taken over by another app — and nothing here ends the call over it.
    /// The stream behind it is rebound to the system default, a camera that stopped is stopped for
    /// the peer as well, and the snapshot carries the reason, so the picker stops claiming a device
    /// that is not there.
    ///
    /// The fresh list is also published when it changed, so a device that came or went shows up in
    /// the pickers without anyone pressing anything.
    pub(super) async fn reconcile_call(&mut self) {
        if self
            .call
            .as_ref()
            .is_none_or(|runtime| !runtime.call.phase().is_live())
        {
            return;
        }
        // Once per heartbeat, the engine's own counters for this call. A call that is up but carrying
        // no audio is otherwise indistinguishable in a bug report from one where nobody spoke, and
        // these numbers are what tell the two apart.
        if let Some(runtime) = self.call.as_ref() {
            runtime.call.log_media_stats();
        }
        // Discovery opens every audio device to name it and runs `v4l2-ctl` with a format probe per
        // camera node, so it runs on a blocking thread rather than on the worker's async loop: a
        // slow device or a stuck helper must not hold up message handling or the call's own events.
        let devices = discover_devices().await;
        // Read before the fresh list replaces it: a device that just went away is still named in
        // here by the description the user saw.
        let known = self.call_devices.clone();
        if devices != known {
            self.call_devices = devices.clone();
            self.emit(Event::CallDevices(Box::new(devices.clone())));
        }
        let mut lost = Vec::new();
        if let Some(runtime) = self.call.as_mut() {
            lost.extend(runtime.call.take_stream_fallbacks());
            lost.extend(runtime.call.verify_devices(&devices).await);
            if runtime.call.camera_stalled()
                && let Err(error) = runtime.call.set_camera(false).await
            {
                log::warn!("[CALL] the peer was not told the camera stopped: {error}");
            }
        }
        if !lost.is_empty() {
            // Worded by the call screen, which knows the reader's language; the log says it plainly.
            let lost: Vec<calls::LostDevice> = lost
                .into_iter()
                .map(|(kind, id)| {
                    let name = calls::device_name(&known, kind, &id);
                    log::warn!("[CALL] {kind:?} \"{name}\" is not available any more");
                    calls::LostDevice { kind, name }
                })
                .collect();
            if let Some(runtime) = self.call.as_mut() {
                runtime.call.set_lost_devices(lost);
            }
        }
        // Compared against the snapshot the UI was handed, so a camera that stopped on its own or a
        // mute another device set reaches the screen even though no user pressed anything.
        let changed = self.call.as_mut().and_then(|runtime| {
            let current = runtime.call.update();
            let changed = current != runtime.last;
            runtime.last = current.clone();
            changed.then_some(current)
        });
        if let Some(update) = changed {
            self.emit_call(update);
        }
    }
}

/// Device discovery off the worker thread: enumerating opens every audio device to name it, so it
/// runs on a blocking thread rather than on the async loop, where a slow device would hold up
/// message handling and the call's own events.
async fn discover_devices() -> calls::DeviceList {
    tokio::task::spawn_blocking(calls::devices)
        .await
        .unwrap_or_default()
}

/// Whether a chat can be called at all.
///
/// The interface only draws the phone and camera buttons on a one-to-one chat, but the worker is the
/// boundary rather than the interface: a group, a channel or a broadcast list reaching the 1:1
/// builder would be refused by the protocol at best and misbehave at worst, so the JID is checked
/// here as well.
fn callable_chat(chat: &str) -> bool {
    matches!(
        crate::model::ChatKind::from_id(chat),
        crate::model::ChatKind::Direct
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_one_to_one_chat_can_be_called() {
        assert!(callable_chat("15551234567@s.whatsapp.net"));
        assert!(callable_chat("123456789012345@lid"));
        assert!(!callable_chat("12345-67890@g.us"));
        assert!(!callable_chat("1234567890@broadcast"));
        assert!(!callable_chat("1234567890@newsletter"));
    }
}

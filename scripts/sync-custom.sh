#!/usr/bin/env bash
set -e

# ==============================================================================
# Script otomatis sinkronisasi ZapFast Upstream dengan Fitur Kustom
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== [1/6] Mengambil commit terbaru dari upstream dan origin ==="
git fetch upstream
git fetch origin

echo "=== [2/6] Memperbarui branch main agar sama dengan upstream/main ==="
git branch -f main upstream/main
git push origin main || echo "Catatan: Push ke origin/main dilewati jika tidak ada perubahan."

echo "=== [3/6] Menggabungkan update upstream ke branch custom-calls ==="
git checkout custom-calls

if git merge upstream/main --no-edit; then
    echo "Merge upstream/main ke custom-calls berhasil tanpa konflik!"
else
    echo ""
    echo "⚠️  ADA MERGE CONFLICT!"
    echo "File yang berkonflik:"
    git diff --name-only --diff-filter=U
    echo ""
    echo "Silakan minta AI (Antigravity): 'Tolong selesaikan konflik merge dan build ulang binary zapfast'"
    exit 1
fi

echo "=== [4/6] Verifikasi kompilasi dan pengujian ==="
OPENSSL_NO_VENDOR=1 cargo check
OPENSSL_NO_VENDOR=1 cargo test stories

echo "=== [5/6] Mengompilasi release binary zapfast-calls ==="
OPENSSL_NO_VENDOR=1 cargo build --release --bin zapfast
install -m 755 target/release/zapfast /home/zekri/.local/bin/zapfast-calls
echo "Binary baru terpasang di /home/zekri/.local/bin/zapfast-calls"

echo "=== [6/6] Push branch custom-calls ke fork GitHub ==="
git push origin custom-calls

echo ""
echo "✅ SEMUA SELESAI! Branch custom-calls telah diperbarui dan disinkronkan dengan upstream."

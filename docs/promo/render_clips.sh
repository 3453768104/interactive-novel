#!/bin/sh
# 片头 / 片尾渲染：背景缓慢推近，文字层保持不动（文字跟着缩放会漂到亮处、发虚）。
# 用法：FFMPEG=<ffmpeg 路径> sh render_clips.sh
set -e
cd "$(dirname "$0")"

FF="${FFMPEG:-ffmpeg}"

intro() {
  still="$1"; title="$2"; out="$3"; w="$4"; h="$5"; dur="$6"; fout="$7"; frames="$8"
  sw=$((w * 12 / 10)); sh=$((h * 12 / 10))
  "$FF" -y -hide_banner -loglevel error -loop 1 -i "$still" -loop 1 -i "$title" -t "$dur" \
    -filter_complex "\
[0:v]scale=${sw}:${sh}:flags=lanczos,zoompan=z='min(1+0.00042*on,1.09)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=${w}x${h}:fps=30,fade=t=in:st=0:d=0.7[bg];\
[1:v]format=rgba,fade=t=in:alpha=1:st=0.55:d=1.1[tx];\
[bg][tx]overlay=0:0,fade=t=out:st=${fout}:d=0.8,format=yuv420p[v]" \
    -map "[v]" -frames:v "$frames" -c:v libx264 -preset slow -crf 18 \
    -movflags +faststart "$out"
  echo "  $out"
}

endcard() {
  card="$1"; out="$2"; w="$3"; h="$4"; dur="$5"; fout="$6"; frames="$7"
  sw=$((w * 12 / 10)); sh=$((h * 12 / 10))
  "$FF" -y -hide_banner -loglevel error -loop 1 -i "$card" -t "$dur" \
    -vf "scale=${sw}:${sh}:flags=lanczos,zoompan=z='min(1+0.00030*on,1.06)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=${w}x${h}:fps=30,fade=t=in:st=0:d=0.6,fade=t=out:st=${fout}:d=0.8,format=yuv420p" \
    -frames:v "$frames" -c:v libx264 -preset slow -crf 18 \
    -movflags +faststart "$out"
  echo "  $out"
}

echo "片头（6s）"
intro still-douyin-1080x1920.png   title-douyin-1080x1920.png  intro-vertical-6s.mp4 1080 1920 6 5.2 180
intro still-16x9-1920x1080.png     title-16x9-1920x1080.png    intro-16x9-6s.mp4     1920 1080 6 5.2 180

echo "片尾（5s）"
endcard endcard-douyin-1080x1920.png endcard-vertical-5s.mp4 1080 1920 5 4.2 150
endcard endcard-16x9-1920x1080.png   endcard-16x9-5s.mp4     1920 1080 5 4.2 150

echo "完成"
ls -lh *.mp4 | awk '{print "  " $9, $5}'

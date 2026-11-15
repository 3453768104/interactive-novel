#!/bin/sh
# 15 秒竖屏预告片 v2：封面 → 真实阅读页 → 真实策略面板 → 片尾。
# 每一段都是「背景缓慢推近 + 内容层静止」，最后用 xfade 叠化。
# 用法：FFMPEG=<ffmpeg 路径> sh render_teaser.sh
set -e
cd "$(dirname "$0")"

FF="${FFMPEG:-ffmpeg}"
W=1080; H=1920
SW=$((W * 12 / 10)); SH=$((H * 12 / 10))

# seg <背景> <内容层> <输出> <帧数> <推近速率>；内容层传 none 表示单图
seg() {
  bg="$1"; fg="$2"; out="$3"; frames="$4"; rate="$5"
  zoom="scale=${SW}:${SH}:flags=lanczos,zoompan=z='min(1+${rate}*on,1.08)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=${W}x${H}:fps=30"
  if [ "$fg" = "none" ]; then
    "$FF" -y -hide_banner -loglevel error -loop 1 -i "$bg" -frames:v "$frames" \
      -vf "${zoom},format=yuv420p" -c:v libx264 -preset medium -crf 18 "$out"
  else
    "$FF" -y -hide_banner -loglevel error -loop 1 -i "$bg" -loop 1 -i "$fg" -frames:v "$frames" \
      -filter_complex "[0:v]${zoom}[b];[b][1:v]overlay=0:0,format=yuv420p[v]" \
      -map "[v]" -c:v libx264 -preset medium -crf 18 "$out"
  fi
  echo "  $out"
}

echo "1/4 封面钩子";          seg still-douyin-1080x1920.png title-douyin-1080x1920.png _s1.mp4 120 0.00040
echo "2/4 真实阅读页";        seg bg-shot-1080x1920.png   shot-reader-1080x1920.png    _s2.mp4 120 0.00036
echo "3/4 真实策略面板";      seg bg-shot-1080x1920.png   shot-settings-1080x1920.png  _s3.mp4 120 0.00036
echo "4/4 片尾";              seg endcard-douyin-1080x1920.png none _s4.mp4 150 0.00030

echo "叠化合成"
"$FF" -y -hide_banner -loglevel error -i _s1.mp4 -i _s2.mp4 -i _s3.mp4 -i _s4.mp4 \
  -filter_complex "\
[0:v][1:v]xfade=transition=fade:duration=0.6:offset=3.4[x1];\
[x1][2:v]xfade=transition=fade:duration=0.6:offset=6.8[x2];\
[x2][3:v]xfade=transition=fade:duration=0.6:offset=10.2[x3];\
[x3]fade=t=out:st=14.4:d=0.8,format=yuv420p[v]" \
  -map "[v]" -c:v libx264 -preset slow -crf 18 -movflags +faststart \
  teaser-vertical-15s.mp4

rm -f _s1.mp4 _s2.mp4 _s3.mp4 _s4.mp4
"$FF" -hide_banner -i teaser-vertical-15s.mp4 2>&1 | grep Duration
ls -lh teaser-vertical-15s.mp4 | awk '{print "  " $9, $5}'

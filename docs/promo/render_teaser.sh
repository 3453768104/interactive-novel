#!/bin/sh
# 15 秒竖屏预告片：封面（钩子）→ 两张文字卡 → 片尾，全部缓慢推近 + 叠化。
# 不需要录屏，可直接发抖音；实机演示片另拍。
# 用法：FFMPEG=<ffmpeg 路径> sh render_teaser.sh
set -e
cd "$(dirname "$0")"

FF="${FFMPEG:-ffmpeg}"
W=1080; H=1920

seg() {
  in="$1"; out="$2"; frames="$3"; rate="$4"
  sw=$((W * 12 / 10)); sh=$((H * 12 / 10))
  "$FF" -y -hide_banner -loglevel error -loop 1 -i "$in" -frames:v "$frames" \
    -vf "scale=${sw}:${sh}:flags=lanczos,zoompan=z='min(1+${rate}*on,1.08)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=${W}x${H}:fps=30,format=yuv420p" \
    -c:v libx264 -preset medium -crf 18 "$out"
}

echo "1/4 钩子（封面）"; seg cover-douyin-1080x1920.png     _seg1.mp4 120 0.00040
echo "2/4 文字卡 A";    seg teaser-b-1080x1920.png         _seg2.mp4 120 0.00040
echo "3/4 文字卡 B";    seg teaser-c-1080x1920.png         _seg3.mp4 120 0.00040
echo "4/4 片尾";        seg endcard-douyin-1080x1920.png   _seg4.mp4 150 0.00030

echo "叠化合成"
"$FF" -y -hide_banner -loglevel error -i _seg1.mp4 -i _seg2.mp4 -i _seg3.mp4 -i _seg4.mp4 \
  -filter_complex "\
[0:v][1:v]xfade=transition=fade:duration=0.6:offset=3.4[x1];\
[x1][2:v]xfade=transition=fade:duration=0.6:offset=6.8[x2];\
[x2][3:v]xfade=transition=fade:duration=0.6:offset=10.2[x3];\
[x3]fade=t=out:st=14.4:d=0.8,format=yuv420p[v]" \
  -map "[v]" -c:v libx264 -preset slow -crf 18 -movflags +faststart \
  teaser-vertical-15s.mp4

rm -f _seg1.mp4 _seg2.mp4 _seg3.mp4 _seg4.mp4
"$FF" -hide_banner -i teaser-vertical-15s.mp4 2>&1 | grep Duration
ls -lh teaser-vertical-15s.mp4 | awk '{print "  " $9, $5}'

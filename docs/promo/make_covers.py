"""把 AI 背景图做成宣传用封面 / 片头静帧。

- AI 出的图不带中文（写中文必糊），所以文字全部在这里用 Windows 字体精确绘制。
- 同时输出「带字封面」和「无字静帧」：后者用于片头关键帧缩放，避免文字跟着缩放糊掉。
"""

import os
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = os.path.dirname(os.path.abspath(__file__))
V = os.path.join(ROOT, "Cinematic_still_life__vertical_2026-11-15T16-38-02.png")
H = os.path.join(ROOT, "Cinematic_still_life__horizont_2026-11-15T16-38-23.png")

F_TITLE = r"C:\Windows\Fonts\simsun.ttc"      # 宋体：书封感
F_UI = r"C:\Windows\Fonts\msyh.ttc"           # 雅黑：说明文字
F_UI_B = r"C:\Windows\Fonts\msyhbd.ttc"

INK = (240, 233, 218)      # 纸白
GOLD = (226, 190, 128)     # 旧金
DIM = (176, 164, 146)      # 次要


def cover_crop(path, size, trim_bottom=0.08):
    """等比缩放填满画布，居中裁切。

    `trim_bottom` 先切掉源图底部那条——生成器的「AI 生成」水印就印在那儿，
    裁掉比贴暗块干净（8% 对构图几乎没有影响）。
    """
    img = Image.open(path).convert("RGB")
    if trim_bottom > 0:
        w, h = img.size
        img = img.crop((0, 0, w, int(h * (1 - trim_bottom))))
    tw, th = size
    sw, sh = img.size
    scale = max(tw / sw, th / sh)
    nw, nh = int(sw * scale + 0.5), int(sh * scale + 0.5)
    img = img.resize((nw, nh), Image.LANCZOS)
    left = (nw - tw) // 2
    top = (nh - th) // 2
    return img.crop((left, top, left + tw, top + th))


def vertical_scrim(img, start_ratio=0.44, bottom_alpha=238):
    """从中间往下压暗，既遮住 ai 水印，也给文字留出干净的底。"""
    w, h = img.size
    grad = Image.new("L", (1, h), 0)
    px = grad.load()
    start = int(h * start_ratio)
    for y in range(start, h):
        t = (y - start) / max(1, h - start)
        px[0, y] = int(bottom_alpha * (t ** 1.45))
    grad = grad.resize((w, h))
    black = Image.new("RGB", (w, h), (8, 7, 6))
    return Image.composite(black, img, grad)


def tracked_text(draw, xy, text, font, fill, tracking=0, stroke=0, stroke_fill=None):
    """逐字绘制以支持字间距（Pillow 没有 tracking）。返回结束 x。"""
    x, y = xy
    for ch in text:
        draw.text((x, y), ch, font=font, fill=fill,
                  stroke_width=stroke, stroke_fill=stroke_fill or fill)
        x += draw.textlength(ch, font=font) + tracking
    return x


def text_width(draw, text, font, tracking=0):
    if not text:
        return 0
    w = sum(draw.textlength(c, font=font) for c in text)
    return w + tracking * (len(text) - 1)


def dark_pad(img, box, radius=110, alpha=92):
    """只在 box 范围内柔和压暗（遮罩式），不动画面其它部分。"""
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rectangle(box, fill=alpha)
    mask = mask.filter(ImageFilter.GaussianBlur(radius))
    return Image.composite(Image.new("RGB", img.size, (0, 0, 0)), img, mask)


def soft_glow(img, box, color=(255, 214, 150), radius=60, alpha=38):
    """在文字后面垫一层暖光，暗底上更好读。"""
    layer = Image.new("RGB", img.size, (0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.rectangle(box, fill=color)
    layer = layer.filter(ImageFilter.GaussianBlur(radius))
    return Image.blend(img, Image.blend(img, layer, alpha / 255.0), 1.0)


def build_vertical():
    W, Hh = 1080, 1920
    base = cover_crop(V, (W, Hh))
    base = vertical_scrim(base)
    base = dark_pad(base, (20, 1000, W - 20, 1620), 110, 96)

    layer = Image.new("RGBA", (W, Hh), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    title1, title2 = "AI 写小说，", "但主角是你"
    sub = "没有大纲 · 每一步都由你决定"
    foot = "Windows 便携版 · 本地离线可跑 · 开源"

    f_t1 = ImageFont.truetype(F_TITLE, 96)
    f_t2 = ImageFont.truetype(F_TITLE, 132)
    f_sub = ImageFont.truetype(F_UI, 40)
    f_foot = ImageFont.truetype(F_UI, 27)

    cx = W // 2
    # 主标题第二行体积更大：视觉重心落在"主角是你"
    y = 1090
    w1 = text_width(d, title1, f_t1, 0)
    tracked_text(d, (cx - w1 / 2, y), title1, f_t1, INK, tracking=0)

    y2 = y + 138
    w2 = text_width(d, title2, f_t2, 8)
    tracked_text(d, (cx - w2 / 2, y2), title2, f_t2, GOLD,
                 tracking=8, stroke=2, stroke_fill=(70, 46, 20))

    # 细金线
    line_y = y2 + 196
    d.line([(cx - 60, line_y), (cx + 60, line_y)], fill=(150, 120, 78), width=2)

    y3 = line_y + 34
    w3 = text_width(d, sub, f_sub, 2)
    tracked_text(d, (cx - w3 / 2, y3), sub, f_sub, DIM, tracking=2)

    w4 = text_width(d, foot, f_foot, 1)
    tracked_text(d, (cx - w4 / 2, Hh - 168), foot, f_foot, (152, 141, 126), tracking=1)

    Image.alpha_composite(base.convert("RGBA"), layer).convert("RGB").save(
        os.path.join(ROOT, "cover-douyin-1080x1920.png"))
    # 单独的文字层：片头里背景慢慢推近，文字保持不动才不会漂到亮处
    layer.save(os.path.join(ROOT, "title-douyin-1080x1920.png"))
    # 无字静帧：片头只做平移缩放，文字用剪映的字幕/贴纸另加，避免缩放糊字
    still = vertical_scrim(cover_crop(V, (W, Hh)), 0.5, 200)
    still = dark_pad(still, (20, 1000, W - 20, 1620), 110, 96)
    still.save(os.path.join(ROOT, "still-douyin-1080x1920.png"))


def build_horizontal():
    W, Hh = 1920, 1080
    base = cover_crop(H, (W, Hh))
    # 左侧再压一点暗，保证文字对比
    veil = Image.new("L", (W, Hh), 0)
    dv = ImageDraw.Draw(veil)
    for x in range(0, int(W * 0.62)):
        a = int(150 * (1 - x / (W * 0.62)) ** 1.2)
        dv.line([(x, 0), (x, Hh)], fill=a)
    base = Image.composite(Image.new("RGB", (W, Hh), (6, 6, 6)), base, veil)
    base = soft_glow(base, (90, 300, 900, 800))

    layer = Image.new("RGBA", (W, Hh), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    f_kicker = ImageFont.truetype(F_UI_B, 30)
    f_t1 = ImageFont.truetype(F_TITLE, 92)
    f_t2 = ImageFont.truetype(F_TITLE, 124)
    f_sub = ImageFont.truetype(F_UI, 36)
    f_foot = ImageFont.truetype(F_UI, 25)

    x0 = 108
    tracked_text(d, (x0, 232), "无 大 纲 互 动 小 说", f_kicker, (198, 158, 96), tracking=3)

    tracked_text(d, (x0, 320), "AI 写小说，", f_t1, INK, tracking=3)
    tracked_text(d, (x0, 448), "但主角是你", f_t2, GOLD, tracking=6,
                 stroke=2, stroke_fill=(72, 48, 22))

    d.line([(x0 + 2, 640), (x0 + 132, 640)], fill=(150, 120, 78), width=2)
    tracked_text(d, (x0 + 2, 676), "每一步都由你决定，AI 只负责把故事写下去", f_sub, DIM, tracking=1)

    tracked_text(d, (x0 + 2, Hh - 116),
                 "Windows 便携版 · 本地离线可跑 · 开源 MIT · 三种模式（本地 / API / 网页粘贴）",
                 f_foot, (150, 139, 124), tracking=1)

    Image.alpha_composite(base.convert("RGBA"), layer).convert("RGB").save(
        os.path.join(ROOT, "cover-bilibili-1920x1080.png"))
    layer.save(os.path.join(ROOT, "title-16x9-1920x1080.png"))
    still = cover_crop(H, (W, Hh))
    still = Image.composite(Image.new("RGB", (W, Hh), (6, 6, 6)), still, veil)
    still.save(os.path.join(ROOT, "still-16x9-1920x1080.png"))


def build_endcard():
    """片尾：留白 + 呼吁，两种画幅各一张。"""
    spec = [
        (V, (1080, 1920), "endcard-douyin-1080x1920.png", 104, 58, 34, 26),
        (H, (1920, 1080), "endcard-16x9-1920x1080.png", 118, 48, 32, 24),
    ]
    for src, size, name, f1, f2, f3, f4 in spec:
        W, Hh = size
        img = cover_crop(src, size)
        # 压成近黑，只留一点点纸纹
        dark = Image.new("RGB", size, (10, 9, 8))
        img = Image.blend(img, dark, 0.90)

        d = ImageDraw.Draw(img)
        cx = W // 2
        f_head = ImageFont.truetype(F_TITLE, f1)
        f_sub = ImageFont.truetype(F_UI, f2)
        f_repo = ImageFont.truetype(F_UI_B, f3)
        f_ver = ImageFont.truetype(F_UI, f4)

        head = "开源免费"
        w = text_width(d, head, f_head, 12)
        y = Hh // 2 - int(Hh * 0.16)
        tracked_text(d, (cx - w / 2, y), head, f_head, INK, tracking=12)

        sub = "本地优先 · 断网也能写 · 数据跟着文件夹走"
        f_sub2 = ImageFont.truetype(F_UI, int(f2 * 0.72))
        w = text_width(d, sub, f_sub2, 1)
        tracked_text(d, (cx - w / 2, y + int(f1 * 1.5)), sub, f_sub2, DIM, tracking=1)

        ly = y + int(f1 * 2.3)
        d.line([(cx - 90, ly), (cx + 90, ly)], fill=(150, 120, 78), width=2)

        repo = "GitHub  ·  interactive-novel"
        w = text_width(d, repo, f_repo, 1)
        tracked_text(d, (cx - w / 2, ly + int(f2 * 0.9)), repo, f_repo, GOLD, tracking=1)

        ver = "v1.0.0  ·  Windows 便携版  ·  MIT"
        w = text_width(d, ver, f_ver, 1)
        tracked_text(d, (cx - w / 2, Hh - int(Hh * 0.09)), ver, f_ver, (120, 110, 98), tracking=1)

        img.save(os.path.join(ROOT, name))


def build_teaser_cards():
    """15 秒预告片用的两张纯文字卡（竖屏），背景是同一张静帧压暗。"""
    cards = [
        ("teaser-b-1080x1920.png", "每一步", "都由你决定"),
        ("teaser-c-1080x1920.png", "写完了", "导出就是一本 EPUB"),
    ]
    for name, l1, l2 in cards:
        img = cover_crop(V, (1080, 1920))
        img = vertical_scrim(img, 0.30, 246)
        img = img.filter(ImageFilter.GaussianBlur(2.2))
        img = dark_pad(img, (0, 0, 1080, 1920), 160, 120)
        d = ImageDraw.Draw(img)
        f1 = ImageFont.truetype(F_TITLE, 84)
        f2 = ImageFont.truetype(F_TITLE, 108)
        cx = 540
        y = 760
        w = text_width(d, l1, f1, 6)
        tracked_text(d, (cx - w / 2, y), l1, f1, INK, tracking=6)
        w = text_width(d, l2, f2, 4)
        tracked_text(d, (cx - w / 2, y + 130), l2, f2, GOLD, tracking=4,
                     stroke=2, stroke_fill=(70, 46, 20))
        d.line([(cx - 54, y + 300), (cx + 54, y + 300)], fill=(150, 120, 78), width=2)
        img.save(os.path.join(ROOT, name))
        print("", name)


if __name__ == "__main__":
    build_vertical()
    build_horizontal()
    build_endcard()
    build_teaser_cards()
    for f in ("cover-douyin-1080x1920.png", "still-douyin-1080x1920.png",
              "cover-bilibili-1920x1080.png", "still-16x9-1920x1080.png",
              "endcard-douyin-1080x1920.png", "endcard-16x9-1920x1080.png"):
        p = os.path.join(ROOT, f)
        print(f, os.path.getsize(p) // 1024, "KB", Image.open(p).size)

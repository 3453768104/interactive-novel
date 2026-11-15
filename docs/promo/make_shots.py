"""把真实界面截图做成竖屏预告片的「画中画」层。

思路：背景（羊皮纸静帧）缓慢推近，界面卡片与字幕单独一层保持静止——
截图里的文字一旦跟着缩放就会发虚，而界面文字恰恰是可信度的来源。

用法：python make_shots.py
产出：shot-reader-1080x1920.png / shot-settings-1080x1920.png、
      bg-shot-1080x1920.png（两张卡片共用的暗背景）
"""

import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.dirname(os.path.abspath(__file__))
SRC = r"C:\Users\34537\.workbuddy\clipboard-images"
SHOT_SETTINGS = os.path.join(SRC, "clipboard-2026-11-15T16-54-33-832Z-36074b69.png")
SHOT_READER = os.path.join(SRC, "clipboard-2026-11-15T16-54-33-833Z-84dcd1ea.png")

STILL = os.path.join(ROOT, "still-douyin-1080x1920.png")
F_TITLE = r"C:\Windows\Fonts\simsun.ttc"
F_UI = r"C:\Windows\Fonts\msyh.ttc"

INK = (240, 233, 218)
GOLD = (226, 190, 128)
W, H = 1080, 1920


def rounded(im, radius, border=(255, 255, 255, 26)):
    """圆角 + 一圈极细的高光边，看着像"贴在纸上的一张屏"。"""
    w, h = im.size
    mask = Image.new("L", (w * 4, h * 4), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, w * 4 - 1, h * 4 - 1), radius=radius * 4, fill=255)
    mask = mask.resize((w, h), Image.LANCZOS)
    out = Image.new("RGBA", (w + 2, h + 2), (0, 0, 0, 0))
    out.paste(im.convert("RGBA"), (1, 1), mask)
    ImageDraw.Draw(out).rounded_rectangle(
        (0, 0, w + 1, h + 1), radius=radius + 1, outline=border, width=1)
    return out


def with_shadow(card, pad=46, blur=26, alpha=190):
    w, h = card.size
    canvas = Image.new("RGBA", (w + pad * 2, h + pad * 2 + 14), (0, 0, 0, 0))
    shadow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        (pad, pad + 16, pad + w, pad + h + 16), radius=22, fill=(0, 0, 0, alpha))
    canvas.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(blur)))
    canvas.alpha_composite(card, (pad, pad))
    return canvas


def crop_shot(path, box, target_w, sharpen=None):
    im = Image.open(path).convert("RGB").crop(box)
    w, h = im.size
    im = im.resize((target_w, int(h * target_w / w)), Image.LANCZOS)
    if sharpen:
        im = im.filter(ImageFilter.UnsharpMask(radius=1.4, percent=sharpen, threshold=2))
    return im


def text_w(draw, text, font, tracking=0):
    if not text:
        return 0
    return sum(draw.textlength(c, font=font) for c in text) + tracking * (len(text) - 1)


def draw_tracked(draw, xy, text, font, fill, tracking=0, stroke=0, stroke_fill=None):
    x, y = xy
    for ch in text:
        draw.text((x, y), ch, font=font, fill=fill,
                  stroke_width=stroke, stroke_fill=stroke_fill or fill)
        x += draw.textlength(ch, font=font) + tracking


def build_background():
    """两张卡片共用的背景：静帧再压暗 + 轻微虚化，让界面卡片跳出来。"""
    base = Image.open(STILL).convert("RGB")
    base = Image.blend(base, Image.new("RGB", base.size, (6, 5, 4)), 0.42)
    base = base.filter(ImageFilter.GaussianBlur(1.6))
    base.save(os.path.join(ROOT, "bg-shot-1080x1920.png"))


def compose(shot_png, box, target_w, cap_top, cap_sub, out_name, sharpen=None, y_center=910):
    card = with_shadow(rounded(crop_shot(shot_png, box, target_w, sharpen), 18))
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    cw, ch = card.size
    x = (W - cw) // 2
    y = y_center - ch // 2
    layer.alpha_composite(card, (x, y))

    d = ImageDraw.Draw(layer)
    f_top = ImageFont.truetype(F_TITLE, 50)
    f_sub = ImageFont.truetype(F_TITLE, 62)

    tw = text_w(d, cap_top, f_top, 6)
    draw_tracked(d, ((W - tw) / 2, y - 96), cap_top, f_top, INK, tracking=6)

    sw = text_w(d, cap_sub, f_sub, 3)
    draw_tracked(d, ((W - sw) / 2, y + ch + 34), cap_sub, f_sub, GOLD, tracking=3,
                 stroke=2, stroke_fill=(64, 42, 18))

    layer.save(os.path.join(ROOT, out_name))
    print(" ", out_name, layer.size)


if __name__ == "__main__":
    build_background()

    # 阅读页：去掉工具栏，只留正文与三个选项（裁掉左侧空白，字更大）
    compose(SHOT_READER, (170, 40, 1286, 872), 1000,
            "它写得像小说", "但下一步，它不替你决定",
            "shot-reader-1080x1920.png", y_center=880)

    # 策略面板：模块额度滑杆（数字与勾选最清楚，最像"真产品"）
    compose(SHOT_SETTINGS, (341, 600, 1157, 1009), 1000,
            "上下文怎么拼", "你说了算",
            "shot-settings-1080x1920.png", sharpen=55, y_center=930)

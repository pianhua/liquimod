import os
import math
from PIL import Image, ImageDraw, ImageFilter

SRC_IMAGE = r"C:\Users\10697\.gemini\antigravity\brain\84ba234c-185d-4b38-afd6-9b38ffbdcd16\liquimod_app_icon_front_1787195930380.jpg"
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS_DIR = os.path.join(ROOT_DIR, "app", "src-tauri", "icons")
STATIC_DIR = os.path.join(ROOT_DIR, "app", "static")

os.makedirs(ICONS_DIR, exist_ok=True)
os.makedirs(STATIC_DIR, exist_ok=True)

def create_squircle_mask(size, radius):
    """创建高抗锯齿连续曲率圆角矩形遮罩"""
    scale = 4
    w, h = size[0] * scale, size[1] * scale
    r = radius * scale
    mask = Image.new("L", (w, h), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle([(0, 0), (w, h)], radius=r, fill=255)
    mask = mask.resize(size, Image.Resampling.LANCZOS)
    return mask

def process_app_icon():
    print(f"Reading source image: {SRC_IMAGE}")
    src = Image.open(SRC_IMAGE).convert("RGBA")

    # 裁切主体居中区域（外框 Squircle 区域）
    # 在 1024x1024 中，图标主体约在 (100, 100, 924, 924)
    crop_box = (100, 100, 924, 924)
    cropped = src.crop(crop_box).resize((1024, 1024), Image.Resampling.LANCZOS)

    # 应用苹果 Squircle 优雅圆角遮罩 (半径约 224px for 1024)
    mask = create_squircle_mask((1024, 1024), radius=220)

    # 创建透明图层
    clean_icon = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    clean_icon.paste(cropped, (0, 0), mask=mask)

    # 1. 导出 512x512 Master PNG
    icon_512 = clean_icon.resize((512, 512), Image.Resampling.LANCZOS)
    icon_512.save(os.path.join(ICONS_DIR, "icon.png"), "PNG")
    print(f"Saved: {os.path.join(ICONS_DIR, 'icon.png')}")

    # 2. 导出 Tauri 规范 PNG
    icon_32 = clean_icon.resize((32, 32), Image.Resampling.LANCZOS)
    icon_32.save(os.path.join(ICONS_DIR, "32x32.png"), "PNG")

    icon_128 = clean_icon.resize((128, 128), Image.Resampling.LANCZOS)
    icon_128.save(os.path.join(ICONS_DIR, "128x128.png"), "PNG")

    icon_256 = clean_icon.resize((256, 256), Image.Resampling.LANCZOS)
    icon_256.save(os.path.join(ICONS_DIR, "128x128@2x.png"), "PNG")
    print("Saved 32x32, 128x128, 128x128@2x PNGs")

    # 3. 导出 Windows Multi-Resolution ICO
    ico_sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (24, 24), (16, 16)]
    ico_images = [clean_icon.resize(s, Image.Resampling.LANCZOS) for s in ico_sizes]
    ico_path = os.path.join(ICONS_DIR, "icon.ico")
    ico_images[0].save(ico_path, format="ICO", sizes=ico_sizes)
    print(f"Saved Windows ICO: {ico_path}")

    # 4. 导出 Web Favicon
    favicon_64 = clean_icon.resize((64, 64), Image.Resampling.LANCZOS)
    favicon_path = os.path.join(STATIC_DIR, "favicon.png")
    favicon_64.save(favicon_path, "PNG")
    print(f"Saved Web Favicon: {favicon_path}")

    # 5. 导出默认 Mod 封面占位图 (Default Mod Cover)
    cover_path = os.path.join(STATIC_DIR, "default_mod_cover.png")
    icon_512.save(cover_path, "PNG")
    print(f"Saved Default Mod Cover: {cover_path}")

if __name__ == "__main__":
    process_app_icon()
    print("All iconography assets generated successfully!")

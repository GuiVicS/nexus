#!/usr/bin/python3 -I
"""Gera as imagens da marca usadas fora da Shell (sem navegador): tela de boot (Plymouth), fundo do sway
enquanto a Shell sobe e os ícones do controle no celular instalado como app (PWA). Saída versionada no repositório; rode de novo só se a marca mudar.
  python3 -I tools/make-brand.py"""
import math
import os
import cairo

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BG = (0x0B / 255, 0x10 / 255, 0x12 / 255)
ACC = (0x2B / 255, 0xD4 / 255, 0xBD / 255)
FG = (0xF3 / 255, 0xFA / 255, 0xF9 / 255)
MUTED = (0xA3 / 255, 0xBA / 255, 0xB7 / 255)


def quad(cr, x1, y1, x2, y2):
    """Curva quadrática (como no SVG) a partir do ponto atual."""
    x0, y0 = cr.get_current_point()
    cr.curve_to(x0 + 2 / 3 * (x1 - x0), y0 + 2 / 3 * (y1 - y0), x2 + 2 / 3 * (x1 - x2), y2 + 2 / 3 * (y1 - y2), x2, y2)


def mark(cr, cx, cy, size):
    """O "N" da marca (mesmo traço do SVG da Shell: viewBox 0..100)."""
    s = size / 100
    cr.save()
    cr.translate(cx - size / 2, cy - size / 2)
    cr.scale(s, s)
    cr.move_to(20, 80)
    cr.line_to(20, 32)
    quad(cr, 20, 16, 32, 26)
    cr.line_to(68, 72)
    quad(cr, 80, 84, 80, 68)
    cr.line_to(80, 20)
    cr.restore()
    cr.set_source_rgb(*ACC)
    cr.set_line_width(9 * s)
    cr.set_line_cap(cairo.LINE_CAP_ROUND)
    cr.set_line_join(cairo.LINE_JOIN_ROUND)
    cr.stroke()


def text(cr, s, cx, y, size, rgb, spacing=0.0, bold=True):
    cr.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
    cr.set_font_size(size)
    widths = [cr.text_extents(c).x_advance for c in s]
    total = sum(widths) + spacing * (len(s) - 1)
    x = cx - total / 2
    cr.set_source_rgb(*rgb)
    for c, w in zip(s, widths):
        cr.move_to(x, y)
        cr.show_text(c)
        x += w + spacing


def splash(path, w, h, sub):
    surf = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
    cr = cairo.Context(surf)
    cr.set_source_rgb(*BG)
    cr.paint()
    u = h / 1080
    mark(cr, w / 2, h / 2 - 70 * u, 190 * u)
    text(cr, "NEXUS", w / 2, h / 2 + 105 * u, 54 * u, FG, spacing=18 * u)
    if sub:
        text(cr, sub, w / 2, h / 2 + 170 * u, 28 * u, MUTED, bold=False)
    surf.write_to_png(path)


def logo(path, size):
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, int(size * 1.45))
    cr = cairo.Context(surf)
    mark(cr, size / 2, size / 2, size * 0.9)
    text(cr, "NEXUS", size / 2, size * 1.32, size * 0.24, FG, spacing=size * 0.08)
    surf.write_to_png(path)


def dot(path, d):
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, d, d)
    cr = cairo.Context(surf)
    cr.arc(d / 2, d / 2, d / 2, 0, 2 * math.pi)
    cr.set_source_rgb(*ACC)
    cr.fill()
    surf.write_to_png(path)


def app_icon(path, size, full_bleed, mark_frac):
    """Ícone do PWA: fundo escuro (quadrado arredondado, ou cheio para 'maskable'/Apple, que cortam sozinhos)."""
    sf = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    cr = cairo.Context(sf)
    cr.set_source_rgb(*BG)
    if full_bleed:
        cr.paint()
    else:
        r = size * 0.22
        cr.new_sub_path()
        cr.arc(size - r, r, r, -math.pi / 2, 0)
        cr.arc(size - r, size - r, r, 0, math.pi / 2)
        cr.arc(r, size - r, r, math.pi / 2, math.pi)
        cr.arc(r, r, r, math.pi, 3 * math.pi / 2)
        cr.close_path()
        cr.fill()
    mark(cr, size / 2, size / 2, size * mark_frac)
    sf.write_to_png(path)


if __name__ == "__main__":
    splash(os.path.join(ROOT, "system/brand/splash.png"), 1920, 1080, "")
    logo(os.path.join(ROOT, "system/plymouth/nexus/logo.png"), 220)
    dot(os.path.join(ROOT, "system/plymouth/nexus/dot.png"), 14)
    pub = os.path.join(ROOT, "apps/remote/public")
    app_icon(os.path.join(pub, "icon-192.png"), 192, False, 0.86)
    app_icon(os.path.join(pub, "icon-512.png"), 512, False, 0.86)
    # maskable: o Android recorta em círculo/gota; a marca fica dentro da zona segura (80 % central)
    app_icon(os.path.join(pub, "icon-maskable-512.png"), 512, True, 0.62)
    app_icon(os.path.join(pub, "apple-touch-icon.png"), 180, True, 0.72)
    print("ok")

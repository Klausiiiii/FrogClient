package dev.frogclient.gui;

import net.minecraft.client.gui.GuiGraphicsExtractor;

/** Shared colors and small drawing helpers for both GUI layouts. */
public final class Theme {
	public static final int ACCENT = 0xFF5FD068;
	public static final int ACCENT_DIM = 0x265FD068;
	public static final int ON_ACCENT = 0xFF0B1A0D;
	public static final int PANEL_BG = 0xF0121814;
	public static final int SUB_BG = 0x40000000;
	public static final int TILE_BG = 0x0DFFFFFF;
	public static final int HOVER = 0x14FFFFFF;
	public static final int TEXT = 0xFFE8F5E9;
	public static final int MUTED = 0xFF8E9A90;
	public static final int SWITCH_OFF = 0xFF3A3F3B;
	public static final int KNOB_OFF = 0xFFAAAAAA;
	public static final int DIM_SCREEN = 0x73000000;

	public static final int SWITCH_W = 20;
	public static final int SWITCH_H = 10;

	private Theme() {
	}

	public static void drawSwitch(GuiGraphicsExtractor g, int x, int y, boolean on) {
		g.fill(x, y, x + SWITCH_W, y + SWITCH_H, on ? ACCENT : SWITCH_OFF);
		int knobX = on ? x + SWITCH_W - SWITCH_H + 2 : x + 2;
		g.fill(knobX, y + 2, knobX + SWITCH_H - 4, y + SWITCH_H - 2, on ? ON_ACCENT : KNOB_OFF);
	}

	public static boolean inside(double mx, double my, int x, int y, int w, int h) {
		return mx >= x && mx < x + w && my >= y && my < y + h;
	}
}

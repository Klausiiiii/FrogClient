package dev.frogclient.hud;

/**
 * HUD element position relative to the free space on screen (0 = left/top edge,
 * 1 = right/bottom edge), so elements stay in place across resizes and GUI scales.
 */
public record HudPosition(double relX, double relY) {
	public static final HudPosition DEFAULT_TOP_LEFT = new HudPosition(0.01, 0.01);

	public int[] toPixels(int screenW, int screenH, int elemW, int elemH) {
		return new int[]{axisToPixels(relX, screenW, elemW), axisToPixels(relY, screenH, elemH)};
	}

	public static HudPosition fromPixels(int x, int y, int screenW, int screenH, int elemW, int elemH) {
		return new HudPosition(axisFromPixels(x, screenW, elemW), axisFromPixels(y, screenH, elemH));
	}

	private static int axisToPixels(double rel, int screen, int elem) {
		int free = screen - elem;
		if (free <= 0) return 0;
		return (int) Math.max(0, Math.min(free, Math.round(rel * free)));
	}

	private static double axisFromPixels(int px, int screen, int elem) {
		int free = screen - elem;
		if (free <= 0) return 0.0;
		return Math.max(0.0, Math.min(1.0, (double) px / free));
	}
}

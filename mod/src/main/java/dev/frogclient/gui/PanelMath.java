package dev.frogclient.gui;

public final class PanelMath {
	private PanelMath() {
	}

	/** Keeps a panel fully on screen; a panel larger than the screen is pinned to the top-left. */
	public static int[] clamp(int x, int y, int panelW, int panelH, int screenW, int screenH) {
		return new int[]{
				Math.max(0, Math.min(x, screenW - panelW)),
				Math.max(0, Math.min(y, screenH - panelH))
		};
	}

	public static int[][] autoArrange(int count, int panelW, int gap, int startX, int startY) {
		int[][] positions = new int[count][];
		for (int i = 0; i < count; i++) {
			positions[i] = new int[]{startX + i * (panelW + gap), startY};
		}
		return positions;
	}
}

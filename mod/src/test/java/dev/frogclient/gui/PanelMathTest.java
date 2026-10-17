package dev.frogclient.gui;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;

class PanelMathTest {
	@Test
	void clampKeepsPanelOnScreen() {
		assertArrayEquals(new int[]{734, 400}, PanelMath.clamp(900, 500, 120, 80, 854, 480));
	}

	@Test
	void clampNegative() {
		assertArrayEquals(new int[]{0, 0}, PanelMath.clamp(-50, -10, 120, 80, 854, 480));
	}

	@Test
	void clampPanelLargerThanScreen() {
		assertArrayEquals(new int[]{0, 0}, PanelMath.clamp(30, 30, 300, 300, 200, 200));
	}

	@Test
	void autoArrangeSideBySide() {
		assertArrayEquals(new int[][]{{10, 14}, {140, 14}, {270, 14}}, PanelMath.autoArrange(3, 120, 10, 10, 14));
	}
}

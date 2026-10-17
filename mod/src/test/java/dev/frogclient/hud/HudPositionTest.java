package dev.frogclient.hud;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class HudPositionTest {
	@Test
	void topLeftDefault() {
		assertArrayEquals(new int[]{19, 11}, HudPosition.DEFAULT_TOP_LEFT.toPixels(1920, 1080, 50, 10));
	}

	@Test
	void clampsWhenScreenShrinks() {
		assertArrayEquals(new int[]{750, 590}, new HudPosition(1, 1).toPixels(800, 600, 50, 10));
		assertArrayEquals(new int[]{750, 590}, new HudPosition(3, 2).toPixels(800, 600, 50, 10));
	}

	@Test
	void elementLargerThanScreen() {
		assertEquals(0, new HudPosition(0.5, 0.5).toPixels(40, 40, 50, 10)[0]);
	}

	@Test
	void roundTripPixels() {
		var p = HudPosition.fromPixels(300, 200, 854, 480, 50, 10);
		int[] px = p.toPixels(854, 480, 50, 10);
		assertTrue(Math.abs(px[0] - 300) <= 1);
		assertTrue(Math.abs(px[1] - 200) <= 1);
	}

	@Test
	void fromPixelsZeroSpace() {
		var p = HudPosition.fromPixels(10, 10, 50, 10, 50, 10);
		assertEquals(0.0, p.relX());
		assertEquals(0.0, p.relY());
	}

	@Test
	void fromPixelsClampsToUnitRange() {
		var p = HudPosition.fromPixels(-100, 5000, 854, 480, 50, 10);
		assertEquals(0.0, p.relX());
		assertEquals(1.0, p.relY());
	}
}

package dev.frogclient.feature.zoom;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class ZoomMathTest {
	@Test
	void fovDividesByFactor() {
		assertEquals(17.5, ZoomMath.fov(70, 4), 1e-9);
	}

	@Test
	void scrollStepsAndClamps() {
		assertEquals(5.0, ZoomMath.scroll(4, 1), 1e-9);
		assertEquals(3.2, ZoomMath.scroll(4, -1), 1e-9);
		assertEquals(50.0, ZoomMath.scroll(49, 3), 1e-9);
		assertEquals(1.5, ZoomMath.scroll(1.6, -1), 1e-9);
		assertEquals(4.0, ZoomMath.scroll(4, 0), 1e-9);
	}

	@Test
	void sensitivityShrinksWithZoom() {
		assertEquals(0.25, ZoomMath.sensitivityMultiplier(4), 1e-9);
	}
}

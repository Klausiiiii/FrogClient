package dev.frogclient.feature.fps;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class FpsModuleTest {
	@Test
	void fpsTextCachedPerValue() {
		var m = new FpsModule();
		assertSame(m.text(144), m.text(144));
		assertEquals("60 FPS", m.text(60));
		assertEquals("144 FPS", m.text(144));
	}

	@Test
	void defaults() {
		var m = new FpsModule();
		assertTrue(m.isEnabled());
		assertEquals("fps", m.hudId());
		assertEquals(1.0, m.scale.get());
		assertEquals(0xFFFFFFFF, m.color.get());
		assertFalse(m.background.get());
	}
}

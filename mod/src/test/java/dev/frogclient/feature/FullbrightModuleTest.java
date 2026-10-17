package dev.frogclient.feature;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class FullbrightModuleTest {
	@Test
	void effectiveGammaOnlyOverridesWhenEnabled() {
		var m = new FullbrightModule();
		assertEquals(0.5, m.effectiveGamma(0.5));
		m.setEnabled(true);
		assertEquals(16.0, m.effectiveGamma(0.5));
		m.setEnabled(false);
		assertEquals(0.5, m.effectiveGamma(0.5));
	}

	@Test
	void fullAmbientOnlyWhenEnabled() {
		var m = new FullbrightModule();
		assertFalse(m.forcesFullAmbient());
		m.setEnabled(true);
		assertTrue(m.forcesFullAmbient());
	}
}

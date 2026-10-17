package dev.frogclient.feature;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
}

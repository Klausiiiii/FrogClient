package dev.frogclient;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class WindowTitleTest {
	@Test
	void instanceNameReplacesTitle() {
		assertEquals("FrogClient – FrogMain", WindowTitle.format("Minecraft* 26.3", "FrogMain"));
	}

	@Test
	void missingOrBlankNameKeepsVanillaTitle() {
		assertEquals("X", WindowTitle.format("X", null));
		assertEquals("X", WindowTitle.format("X", " "));
	}
}

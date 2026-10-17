package dev.frogclient.module;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ModuleTest {
	static final class Counting extends Module {
		int enables;
		int disables;

		Counting(String id, Category category) {
			super(id, category, false);
		}

		@Override protected void onEnable() { enables++; }
		@Override protected void onDisable() { disables++; }
	}

	@Test
	void setEnabledCallsHooksOnlyOnChange() {
		var m = new Counting("a", Category.RENDER);
		m.setEnabled(true);
		m.setEnabled(true);
		assertEquals(1, m.enables);
		m.toggle();
		assertFalse(m.isEnabled());
		assertEquals(1, m.disables);
	}

	@Test
	void byCategoryKeepsRegistrationOrder() {
		var mm = new ModuleManager();
		var b = new Counting("b", Category.RENDER);
		var h = new Counting("h", Category.HUD);
		var a = new Counting("a", Category.RENDER);
		mm.register(b);
		mm.register(h);
		mm.register(a);
		assertEquals(List.of(b, a), mm.byCategory(Category.RENDER));
		assertEquals(List.of(b, h, a), mm.all());
	}

	@Test
	void keysDerivedFromId() {
		var m = new Counting("zoom", Category.RENDER);
		assertEquals("frogclient.module.zoom", m.nameKey());
		assertEquals("frogclient.category.hud", Category.HUD.nameKey());
	}
}

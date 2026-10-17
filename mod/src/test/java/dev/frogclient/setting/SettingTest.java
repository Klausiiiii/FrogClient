package dev.frogclient.setting;

import com.google.gson.JsonNull;
import com.google.gson.JsonPrimitive;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class SettingTest {
	enum TestEnum { A, B }

	@Test
	void sliderClampsAndSnaps() {
		var s = new SliderSetting("factor", "k", 2, 10, 0.5, 4);
		s.set(999.0);
		assertEquals(10.0, s.get());
		s.set(-3.0);
		assertEquals(2.0, s.get());
		s.set(4.3);
		assertEquals(4.5, s.get());
	}

	@Test
	void sliderFromJsonClampsOutOfRange() {
		var s = new SliderSetting("factor", "k", 2, 10, 0.5, 4);
		s.fromJson(new JsonPrimitive(999));
		assertEquals(10.0, s.get());
	}

	@Test
	void invalidJsonFallsBackToDefault() {
		var b = new BoolSetting("x", "k", true);
		b.set(false);
		b.fromJson(new JsonPrimitive("kaputt"));
		assertTrue(b.get());

		var e = new EnumSetting<>("e", "k", TestEnum.class, TestEnum.A);
		e.set(TestEnum.B);
		e.fromJson(new JsonPrimitive("GIBTSNICHT"));
		assertEquals(TestEnum.A, e.get());

		var c = new ColorSetting("c", "k", 0xFFFFFFFF);
		c.set(0xFF000000);
		c.fromJson(JsonNull.INSTANCE);
		assertEquals(0xFFFFFFFF, c.get());

		var s = new SliderSetting("s", "k", 0, 1, 0.1, 0.5);
		s.set(0.9);
		s.fromJson(null);
		assertEquals(0.5, s.get());
	}

	@Test
	void roundTripAllTypes() {
		var b = new BoolSetting("b", "k", true);
		b.set(false);
		var b2 = new BoolSetting("b", "k", true);
		b2.fromJson(b.toJson());
		assertEquals(false, b2.get());

		var s = new SliderSetting("s", "k", 2, 10, 0.5, 4);
		s.set(7.5);
		var s2 = new SliderSetting("s", "k", 2, 10, 0.5, 4);
		s2.fromJson(s.toJson());
		assertEquals(7.5, s2.get());

		var e = new EnumSetting<>("e", "k", TestEnum.class, TestEnum.A);
		e.set(TestEnum.B);
		var e2 = new EnumSetting<>("e", "k", TestEnum.class, TestEnum.A);
		e2.fromJson(e.toJson());
		assertEquals(TestEnum.B, e2.get());

		var c = new ColorSetting("c", "k", 0xFFFFFFFF);
		c.set(0x80FF5555);
		var c2 = new ColorSetting("c", "k", 0xFFFFFFFF);
		c2.fromJson(c.toJson());
		assertEquals(0x80FF5555, c2.get());
	}

	@Test
	void colorJsonFormat() {
		assertEquals("#FF5FD068", new ColorSetting("c", "k", 0xFF5FD068).toJson().getAsString());
	}

	@Test
	void enumNextCycles() {
		var e = new EnumSetting<>("e", "k", TestEnum.class, TestEnum.A);
		assertEquals(TestEnum.B, e.next());
		assertEquals(TestEnum.A, e.next());
		assertEquals(TestEnum.A, e.get());
	}

	@Test
	void colorNextCyclesPresets() {
		var c = new ColorSetting("c", "k", 0xFFFFFFFF);
		assertEquals(0xFF5FD068, c.next());
		c.set(ColorSetting.PRESETS[ColorSetting.PRESETS.length - 1]);
		assertEquals(ColorSetting.PRESETS[0], c.next());
		c.set(0xFF123456);
		assertEquals(ColorSetting.PRESETS[0], c.next());
	}

	@Test
	void resetRestoresDefault() {
		var s = new SliderSetting("s", "k", 2, 10, 0.5, 4);
		s.set(9.0);
		s.reset();
		assertEquals(4.0, s.get());
	}
}

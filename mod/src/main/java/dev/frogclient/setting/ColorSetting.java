package dev.frogclient.setting;

import com.google.gson.JsonElement;
import com.google.gson.JsonPrimitive;

/** ARGB color, persisted as "#AARRGGBB". The GUI cycles through {@link #PRESETS}. */
public final class ColorSetting implements Setting<Integer> {
	public static final int[] PRESETS = {
			0xFFFFFFFF, 0xFF5FD068, 0xFFFF5555, 0xFFFFAA00,
			0xFFFFFF55, 0xFF55FFFF, 0xFF5555FF, 0xFFFF55FF
	};

	private final String id;
	private final String nameKey;
	private final int def;
	private int value;

	public ColorSetting(String id, String nameKey, int defArgb) {
		this.id = id;
		this.nameKey = nameKey;
		this.def = defArgb;
		this.value = defArgb;
	}

	@Override public String id() { return id; }
	@Override public String nameKey() { return nameKey; }
	@Override public Integer get() { return value; }
	@Override public void set(Integer value) { this.value = value != null ? value : def; }
	@Override public Integer defaultValue() { return def; }

	public int argb() {
		return value;
	}

	/** Jumps to the preset after the current one; a non-preset value jumps to the first preset. */
	public int next() {
		int index = -1;
		for (int i = 0; i < PRESETS.length; i++) {
			if (PRESETS[i] == value) {
				index = i;
				break;
			}
		}
		value = PRESETS[(index + 1) % PRESETS.length];
		return value;
	}

	@Override
	public JsonElement toJson() {
		return new JsonPrimitive(String.format("#%08X", value));
	}

	@Override
	public void fromJson(JsonElement json) {
		value = def;
		if (json == null || !json.isJsonPrimitive() || !json.getAsJsonPrimitive().isString()) return;
		String s = json.getAsString();
		if (s.length() != 9 || s.charAt(0) != '#') return;
		try {
			value = (int) Long.parseLong(s.substring(1), 16);
		} catch (NumberFormatException ignored) {
			// keep default
		}
	}
}

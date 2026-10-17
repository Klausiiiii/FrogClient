package dev.frogclient.setting;

import com.google.gson.JsonElement;

/**
 * A single declarative, persistable module setting. Both GUI layouts and the
 * config store work exclusively from these declarations.
 */
public sealed interface Setting<T> permits BoolSetting, SliderSetting, EnumSetting, ColorSetting {
	String id();

	String nameKey();

	T get();

	void set(T value);

	T defaultValue();

	default void reset() {
		set(defaultValue());
	}

	JsonElement toJson();

	/** Reads a value; anything missing or invalid resets to the default. Never throws. */
	void fromJson(JsonElement json);
}

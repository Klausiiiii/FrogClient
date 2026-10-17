package dev.frogclient.setting;

import com.google.gson.JsonElement;
import com.google.gson.JsonPrimitive;

public final class BoolSetting implements Setting<Boolean> {
	private final String id;
	private final String nameKey;
	private final boolean def;
	private boolean value;

	public BoolSetting(String id, String nameKey, boolean def) {
		this.id = id;
		this.nameKey = nameKey;
		this.def = def;
		this.value = def;
	}

	@Override public String id() { return id; }
	@Override public String nameKey() { return nameKey; }
	@Override public Boolean get() { return value; }
	@Override public void set(Boolean value) { this.value = value != null ? value : def; }
	@Override public Boolean defaultValue() { return def; }

	public boolean isOn() {
		return value;
	}

	public void toggle() {
		value = !value;
	}

	@Override
	public JsonElement toJson() {
		return new JsonPrimitive(value);
	}

	@Override
	public void fromJson(JsonElement json) {
		if (json != null && json.isJsonPrimitive() && json.getAsJsonPrimitive().isBoolean()) {
			value = json.getAsBoolean();
		} else {
			value = def;
		}
	}
}

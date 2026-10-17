package dev.frogclient.setting;

import com.google.gson.JsonElement;
import com.google.gson.JsonPrimitive;

public final class EnumSetting<E extends Enum<E>> implements Setting<E> {
	private final String id;
	private final String nameKey;
	private final Class<E> type;
	private final E def;
	private E value;

	public EnumSetting(String id, String nameKey, Class<E> type, E def) {
		this.id = id;
		this.nameKey = nameKey;
		this.type = type;
		this.def = def;
		this.value = def;
	}

	@Override public String id() { return id; }
	@Override public String nameKey() { return nameKey; }
	@Override public E get() { return value; }
	@Override public void set(E value) { this.value = value != null ? value : def; }
	@Override public E defaultValue() { return def; }

	public Class<E> type() {
		return type;
	}

	/** Advances to the next constant (wrapping) and returns it. */
	public E next() {
		E[] all = type.getEnumConstants();
		value = all[(value.ordinal() + 1) % all.length];
		return value;
	}

	@Override
	public JsonElement toJson() {
		return new JsonPrimitive(value.name());
	}

	@Override
	public void fromJson(JsonElement json) {
		value = def;
		if (json == null || !json.isJsonPrimitive() || !json.getAsJsonPrimitive().isString()) return;
		String name = json.getAsString();
		for (E constant : type.getEnumConstants()) {
			if (constant.name().equals(name)) {
				value = constant;
				return;
			}
		}
	}
}

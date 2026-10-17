package dev.frogclient.setting;

import com.google.gson.JsonElement;
import com.google.gson.JsonPrimitive;

public final class SliderSetting implements Setting<Double> {
	private final String id;
	private final String nameKey;
	private final double min;
	private final double max;
	private final double step;
	private final double def;
	private double value;

	public SliderSetting(String id, String nameKey, double min, double max, double step, double def) {
		this.id = id;
		this.nameKey = nameKey;
		this.min = min;
		this.max = max;
		this.step = step;
		this.def = def;
		this.value = normalize(def);
	}

	@Override public String id() { return id; }
	@Override public String nameKey() { return nameKey; }
	@Override public Double get() { return value; }
	@Override public Double defaultValue() { return def; }

	public double min() { return min; }
	public double max() { return max; }
	public double step() { return step; }

	public double value() {
		return value;
	}

	@Override
	public void set(Double value) {
		this.value = value == null || value.isNaN() ? normalize(def) : normalize(value);
	}

	private double normalize(double v) {
		double clamped = Math.max(min, Math.min(max, v));
		double snapped = min + Math.round((clamped - min) / step) * step;
		// Avoid floating noise like 4.500000000001 and stay inside the range after rounding.
		snapped = Math.round(snapped * 1e9) / 1e9;
		return Math.max(min, Math.min(max, snapped));
	}

	@Override
	public JsonElement toJson() {
		return new JsonPrimitive(value);
	}

	@Override
	public void fromJson(JsonElement json) {
		if (json != null && json.isJsonPrimitive() && json.getAsJsonPrimitive().isNumber()) {
			set(json.getAsDouble());
		} else {
			reset();
		}
	}
}

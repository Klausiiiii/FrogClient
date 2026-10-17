package dev.frogclient.module;

import java.util.Locale;

public enum Category {
	RENDER, HUD, SETTINGS;

	public String nameKey() {
		return "frogclient.category." + name().toLowerCase(Locale.ROOT);
	}
}

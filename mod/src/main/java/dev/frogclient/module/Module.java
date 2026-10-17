package dev.frogclient.module;

import dev.frogclient.setting.Setting;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * A client feature. Its settings are declared once via {@link #add} and rendered
 * by every GUI layout and persisted by the config store from that declaration.
 */
public abstract class Module {
	private final String id;
	private final Category category;
	private final boolean defaultEnabled;
	private final List<Setting<?>> settings = new ArrayList<>();
	private boolean enabled;

	protected Module(String id, Category category, boolean defaultEnabled) {
		this.id = id;
		this.category = category;
		this.defaultEnabled = defaultEnabled;
		this.enabled = defaultEnabled;
	}

	public String id() {
		return id;
	}

	public String nameKey() {
		return "frogclient.module." + id;
	}

	public Category category() {
		return category;
	}

	public boolean defaultEnabled() {
		return defaultEnabled;
	}

	public boolean isEnabled() {
		return enabled;
	}

	public void setEnabled(boolean enabled) {
		if (this.enabled == enabled) return;
		this.enabled = enabled;
		if (enabled) onEnable();
		else onDisable();
	}

	public void toggle() {
		setEnabled(!enabled);
	}

	public List<Setting<?>> settings() {
		return Collections.unmodifiableList(settings);
	}

	protected <S extends Setting<?>> S add(S setting) {
		settings.add(setting);
		return setting;
	}

	protected void onEnable() {
	}

	protected void onDisable() {
	}
}

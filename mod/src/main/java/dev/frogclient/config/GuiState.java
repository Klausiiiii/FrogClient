package dev.frogclient.config;

import dev.frogclient.module.Category;

import java.util.EnumMap;
import java.util.HashMap;
import java.util.Map;

/** Persisted GUI state: chosen layout, panel positions (GUI pixels) and HUD positions (relative 0..1). */
public final class GuiState {
	public enum Layout { PANELS, WINDOW }

	private Layout layout = Layout.PANELS;
	private final Map<Category, int[]> panelPositions = new EnumMap<>(Category.class);
	private final Map<String, double[]> hudPositions = new HashMap<>();

	public Layout layout() {
		return layout;
	}

	public void setLayout(Layout layout) {
		this.layout = layout;
	}

	public Map<Category, int[]> panelPositions() {
		return panelPositions;
	}

	public Map<String, double[]> hudPositions() {
		return hudPositions;
	}
}

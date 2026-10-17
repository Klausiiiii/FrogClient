package dev.frogclient.hud;

import net.minecraft.client.gui.GuiGraphicsExtractor;

/** A movable HUD element. Width/height are in GUI pixels and already include the element's own scale. */
public interface HudElement {
	String hudId();

	int width();

	int height();

	void render(GuiGraphicsExtractor g, int x, int y);

	boolean visible();
}

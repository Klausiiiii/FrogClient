package dev.frogclient.gui;

import net.minecraft.client.gui.GuiGraphicsExtractor;

public interface GuiLayout {
	/** Called on open and after every resize. */
	void init(ClientScreen screen, int width, int height);

	void render(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta);

	boolean mouseClicked(double mx, double my, int button);

	boolean mouseDragged(double mx, double my, int button, double dx, double dy);

	boolean mouseReleased(double mx, double my, int button);

	boolean mouseScrolled(double mx, double my, double scrollY);
}

package dev.frogclient.gui;

import dev.frogclient.FrogClient;
import dev.frogclient.config.GuiState;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

public final class ClientScreen extends Screen {
	private static final int BUTTON_W = 100;
	private static final int BUTTON_H = 16;

	private GuiLayout layout;

	public ClientScreen() {
		super(Component.literal("FrogClient"));
	}

	@Override
	protected void init() {
		if (layout == null) layout = createLayout();
		layout.init(this, width, height);

		addRenderableWidget(Button.builder(layoutLabel(), b -> switchLayout())
				.bounds(width - BUTTON_W - 6, 6, BUTTON_W, BUTTON_H).build());
	}

	private static GuiLayout createLayout() {
		return FrogClient.gui.layout() == GuiState.Layout.WINDOW ? new WindowLayout() : new PanelLayout();
	}

	private static Component layoutLabel() {
		return Component.translatable(FrogClient.gui.layout() == GuiState.Layout.WINDOW
				? "frogclient.gui.layout.window" : "frogclient.gui.layout.panels");
	}

	private void switchLayout() {
		FrogClient.gui.setLayout(FrogClient.gui.layout() == GuiState.Layout.PANELS ? GuiState.Layout.WINDOW : GuiState.Layout.PANELS);
		layout = createLayout();
		rebuildWidgets();
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		g.fill(0, 0, width, height, Theme.DIM_SCREEN);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		layout.render(g, mouseX, mouseY, a);
		super.extractRenderState(g, mouseX, mouseY, a);
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		if (super.mouseClicked(event, doubleClick)) return true;
		return layout.mouseClicked(event.x(), event.y(), event.button());
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
		if (layout.mouseDragged(event.x(), event.y(), event.button(), dx, dy)) return true;
		return super.mouseDragged(event, dx, dy);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		if (layout.mouseReleased(event.x(), event.y(), event.button())) return true;
		return super.mouseReleased(event);
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		if (layout.mouseScrolled(x, y, scrollY)) return true;
		return super.mouseScrolled(x, y, scrollX, scrollY);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	@Override
	public void onClose() {
		FrogClient.saveConfig();
		super.onClose();
	}
}

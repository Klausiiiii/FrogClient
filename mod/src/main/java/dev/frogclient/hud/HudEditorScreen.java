package dev.frogclient.hud;

import dev.frogclient.FrogClient;
import dev.frogclient.gui.Theme;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

import java.util.List;

/** Drag HUD elements to move them; right click resets one to the default position. */
public final class HudEditorScreen extends Screen {
	private final Screen parent;
	private final List<HudElement> elements;
	private HudElement dragging;
	private int dragOffX;
	private int dragOffY;

	public HudEditorScreen(Screen parent) {
		super(Component.translatable("frogclient.gui.edit_hud"));
		this.parent = parent;
		this.elements = FrogClient.modules.all().stream()
				.filter(m -> m instanceof HudElement)
				.map(m -> (HudElement) m)
				.toList();
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		g.fill(0, 0, width, height, 0x40000000);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		for (HudElement e : elements) {
			if (!e.visible()) continue;
			int[] px = HudRenderer.renderAt(g, e, FrogClient.gui);
			g.outline(px[0] - 1, px[1] - 1, e.width() + 2, e.height() + 2, Theme.ACCENT);
		}
		g.centeredText(font, Component.translatable("frogclient.gui.hud_hint"), width / 2, height - 20, Theme.TEXT);
		super.extractRenderState(g, mouseX, mouseY, a);
	}

	private HudElement elementAt(double mx, double my) {
		for (HudElement e : elements) {
			if (!e.visible()) continue;
			int[] px = HudRenderer.position(e, FrogClient.gui).toPixels(width, height, e.width(), e.height());
			if (Theme.inside(mx, my, px[0], px[1], e.width(), e.height())) return e;
		}
		return null;
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		HudElement e = elementAt(event.x(), event.y());
		if (e == null) return super.mouseClicked(event, doubleClick);
		if (event.button() == 1) {
			FrogClient.gui.hudPositions().remove(e.hudId());
			return true;
		}
		if (event.button() == 0) {
			int[] px = HudRenderer.position(e, FrogClient.gui).toPixels(width, height, e.width(), e.height());
			dragging = e;
			dragOffX = (int) event.x() - px[0];
			dragOffY = (int) event.y() - px[1];
		}
		return true;
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
		if (dragging == null) return super.mouseDragged(event, dx, dy);
		HudPosition p = HudPosition.fromPixels((int) event.x() - dragOffX, (int) event.y() - dragOffY,
				width, height, dragging.width(), dragging.height());
		FrogClient.gui.hudPositions().put(dragging.hudId(), new double[]{p.relX(), p.relY()});
		return true;
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		if (dragging == null) return super.mouseReleased(event);
		dragging = null;
		return true;
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	@Override
	public void onClose() {
		FrogClient.saveConfig();
		minecraft.gui.setScreen(parent);
	}
}

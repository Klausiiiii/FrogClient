package dev.frogclient.gui;

import dev.frogclient.FrogClient;
import dev.frogclient.gui.widget.SettingWidget;
import dev.frogclient.gui.widget.WidgetFactory;
import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

import java.util.ArrayList;
import java.util.List;

/** Layout B: centered window, categories on the left, module tiles on the right. */
public final class WindowLayout implements GuiLayout {
	private static final int PAD = 10;
	private static final int CAT_H = 16;
	private static final int TILE_H = 24;
	private static final int TILE_GAP = 8;
	private static final int BACK_H = 16;

	private final List<Category> categories = new ArrayList<>();
	private int wx;
	private int wy;
	private int ww;
	private int wh;
	private int sideW;
	private Category selected;
	private Module openModule;
	private List<SettingWidget> openWidgets = List.of();
	private SettingWidget activeWidget;
	private double scroll;
	private int contentHeight;

	@Override
	public void init(ClientScreen screen, int width, int height) {
		ww = (int) (width * 0.82);
		wx = (width - ww) / 2;
		// Keep the window below the layout/HUD buttons in the top-right corner.
		wy = Math.max((int) (height * 0.12), ClientScreen.BUTTONS_BOTTOM + 6);
		wh = Math.max(60, Math.min((int) (height * 0.76), height - wy - 6));
		sideW = (int) (ww * 0.28);
		categories.clear();
		for (Category c : Category.values()) {
			if (!FrogClient.modules.byCategory(c).isEmpty()) categories.add(c);
		}
		if (selected == null || !categories.contains(selected)) {
			selected = categories.isEmpty() ? null : categories.getFirst();
		}
		clampScroll();
	}

	private int mainX() {
		return wx + sideW + PAD;
	}

	private int mainY() {
		return wy + PAD;
	}

	private int mainW() {
		return ww - sideW - 2 * PAD;
	}

	private int mainH() {
		return wh - 2 * PAD;
	}

	private int tileW() {
		return (mainW() - TILE_GAP) / 2;
	}

	private void clampScroll() {
		scroll = Math.max(0, Math.min(scroll, Math.max(0, contentHeight - mainH())));
	}

	@Override
	public void render(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		var font = Minecraft.getInstance().font;
		g.fill(wx, wy, wx + ww, wy + wh, Theme.PANEL_BG);
		g.fill(wx + sideW, wy, wx + sideW + 1, wy + wh, Theme.HOVER);

		g.text(font, "FrogClient", wx + PAD, wy + PAD, Theme.ACCENT, false);
		int cy = wy + PAD + 16;
		for (Category c : categories) {
			boolean active = c == selected;
			if (active) g.fill(wx + 6, cy, wx + sideW - 6, cy + CAT_H, Theme.ACCENT_DIM);
			else if (Theme.inside(mouseX, mouseY, wx + 6, cy, sideW - 12, CAT_H)) g.fill(wx + 6, cy, wx + sideW - 6, cy + CAT_H, Theme.HOVER);
			g.text(font, Component.translatable(c.nameKey()), wx + 12, cy + 4, active ? Theme.ACCENT : Theme.TEXT, false);
			cy += CAT_H + 2;
		}

		g.enableScissor(mainX(), mainY(), mainX() + mainW(), mainY() + mainH());
		int top = mainY() - (int) scroll;
		if (openModule != null) renderSettings(g, font, top, mouseX, mouseY);
		else renderTiles(g, font, top, mouseX, mouseY);
		g.disableScissor();
	}

	private void renderTiles(GuiGraphicsExtractor g, Font font, int top, int mouseX, int mouseY) {
		if (selected == null) return;
		List<Module> mods = FrogClient.modules.byCategory(selected);
		for (int i = 0; i < mods.size(); i++) {
			Module m = mods.get(i);
			int tx = mainX() + (i % 2) * (tileW() + TILE_GAP);
			int ty = top + (i / 2) * (TILE_H + TILE_GAP);
			g.fill(tx, ty, tx + tileW(), ty + TILE_H, Theme.inside(mouseX, mouseY, tx, ty, tileW(), TILE_H) ? Theme.HOVER : Theme.TILE_BG);
			g.text(font, Component.translatable(m.nameKey()), tx + 8, ty + 8, Theme.TEXT, false);
			Theme.drawSwitch(g, tx + tileW() - Theme.SWITCH_W - 8, ty + 7, m.isEnabled());
		}
		contentHeight = ((mods.size() + 1) / 2) * (TILE_H + TILE_GAP);
	}

	private void renderSettings(GuiGraphicsExtractor g, Font font, int top, int mouseX, int mouseY) {
		int x = mainX();
		g.text(font, Component.translatable("frogclient.gui.back"), x, top + 4,
				Theme.inside(mouseX, mouseY, x, top, 60, BACK_H) ? Theme.ACCENT : Theme.MUTED, false);
		int y = top + BACK_H + 4;
		g.text(font, Component.translatable(openModule.nameKey()), x, y + 3, Theme.ACCENT, false);
		Theme.drawSwitch(g, x + mainW() - Theme.SWITCH_W, y + 2, openModule.isEnabled());
		y += 18;
		for (SettingWidget w : openWidgets) {
			w.render(g, x, y, mainW(), mouseX, mouseY);
			y += w.height() + 2;
		}
		contentHeight = y - top;
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (!Theme.inside(mx, my, wx, wy, ww, wh)) return false;
		if (button != 0) return true;

		int cy = wy + PAD + 16;
		for (Category c : categories) {
			if (Theme.inside(mx, my, wx + 6, cy, sideW - 12, CAT_H)) {
				selected = c;
				openModule = null;
				scroll = 0;
				return true;
			}
			cy += CAT_H + 2;
		}

		if (!Theme.inside(mx, my, mainX(), mainY(), mainW(), mainH())) return true;
		int top = mainY() - (int) scroll;

		if (openModule != null) {
			if (Theme.inside(mx, my, mainX(), top, 60, BACK_H)) {
				openModule = null;
				scroll = 0;
				return true;
			}
			int headerY = top + BACK_H + 4;
			if (Theme.inside(mx, my, mainX(), headerY, mainW(), 16)) {
				openModule.toggle();
				return true;
			}
			for (SettingWidget w : openWidgets) {
				if (w.mouseClicked(mx, my, button)) {
					activeWidget = w;
					return true;
				}
			}
			return true;
		}

		if (selected == null) return true;
		List<Module> mods = FrogClient.modules.byCategory(selected);
		for (int i = 0; i < mods.size(); i++) {
			int tx = mainX() + (i % 2) * (tileW() + TILE_GAP);
			int ty = top + (i / 2) * (TILE_H + TILE_GAP);
			if (!Theme.inside(mx, my, tx, ty, tileW(), TILE_H)) continue;
			Module m = mods.get(i);
			boolean onSwitch = mx >= tx + tileW() - Theme.SWITCH_W - 12;
			if (onSwitch || m.settings().isEmpty()) {
				m.toggle();
			} else {
				openModule = m;
				openWidgets = WidgetFactory.forModule(m);
				scroll = 0;
			}
			return true;
		}
		return true;
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button, double dx, double dy) {
		return activeWidget != null && activeWidget.mouseDragged(mx, my);
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (activeWidget == null) return false;
		activeWidget.mouseReleased();
		activeWidget = null;
		return true;
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double scrollY) {
		if (!Theme.inside(mx, my, mainX(), mainY(), mainW(), mainH())) return false;
		scroll -= scrollY * 12;
		clampScroll();
		return true;
	}
}

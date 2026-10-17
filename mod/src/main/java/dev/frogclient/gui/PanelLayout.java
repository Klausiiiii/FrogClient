package dev.frogclient.gui;

import dev.frogclient.FrogClient;
import dev.frogclient.gui.widget.SettingWidget;
import dev.frogclient.gui.widget.WidgetFactory;
import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Layout A: one draggable panel per category. Left click toggles, right click expands settings. */
public final class PanelLayout implements GuiLayout {
	static final int PANEL_W = 120;
	private static final int HEADER_H = 16;
	private static final int ROW_H = 14;
	private static final int GAP = 10;
	private static final int START_X = 10;
	private static final int START_Y = 10;

	private final List<Panel> panels = new ArrayList<>();
	private final Set<Module> expanded = new HashSet<>();
	private final Map<Module, List<SettingWidget>> widgets = new HashMap<>();
	private int screenW;
	private int screenH;
	private Panel dragging;
	private int dragOffX;
	private int dragOffY;
	private SettingWidget activeWidget;

	private static final class Panel {
		final Category category;
		final List<Module> modules;
		int x;
		int y;

		Panel(Category category, List<Module> modules) {
			this.category = category;
			this.modules = modules;
		}
	}

	@Override
	public void init(ClientScreen screen, int width, int height) {
		screenW = width;
		screenH = height;
		panels.clear();
		for (Category c : Category.values()) {
			List<Module> mods = FrogClient.modules.byCategory(c);
			if (!mods.isEmpty()) panels.add(new Panel(c, mods));
		}
		int[][] auto = PanelMath.autoArrange(panels.size(), PANEL_W, GAP, START_X, START_Y);
		for (int i = 0; i < panels.size(); i++) {
			Panel p = panels.get(i);
			int[] saved = FrogClient.gui.panelPositions().get(p.category);
			int[] pos = saved != null ? saved : auto[i];
			int[] clamped = PanelMath.clamp(pos[0], pos[1], PANEL_W, HEADER_H, width, height);
			p.x = clamped[0];
			p.y = clamped[1];
		}
	}

	private List<SettingWidget> widgetsOf(Module m) {
		return widgets.computeIfAbsent(m, WidgetFactory::forModule);
	}

	private int panelHeight(Panel p) {
		int h = HEADER_H;
		for (Module m : p.modules) {
			h += ROW_H;
			if (expanded.contains(m)) {
				for (SettingWidget w : widgetsOf(m)) h += w.height();
				h += 2;
			}
		}
		return h + 2;
	}

	@Override
	public void render(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		var font = Minecraft.getInstance().font;
		for (Panel p : panels) {
			g.fill(p.x, p.y, p.x + PANEL_W, p.y + panelHeight(p), Theme.PANEL_BG);
			g.fill(p.x, p.y, p.x + PANEL_W, p.y + HEADER_H, Theme.ACCENT);
			g.text(font, Component.translatable(p.category.nameKey()), p.x + 6, p.y + 4, Theme.ON_ACCENT, false);

			int y = p.y + HEADER_H;
			for (Module m : p.modules) {
				if (Theme.inside(mouseX, mouseY, p.x, y, PANEL_W, ROW_H)) {
					g.fill(p.x, y, p.x + PANEL_W, y + ROW_H, Theme.HOVER);
				}
				g.text(font, Component.translatable(m.nameKey()), p.x + 6, y + 3,
						m.isEnabled() ? Theme.ACCENT : Theme.TEXT, false);
				if (!m.settings().isEmpty()) {
					g.text(font, expanded.contains(m) ? "-" : "+", p.x + PANEL_W - 10, y + 3, Theme.MUTED, false);
				}
				y += ROW_H;
				if (expanded.contains(m)) {
					int start = y;
					int subH = 2;
					for (SettingWidget w : widgetsOf(m)) subH += w.height();
					g.fill(p.x, start, p.x + PANEL_W, start + subH, Theme.SUB_BG);
					for (SettingWidget w : widgetsOf(m)) {
						w.render(g, p.x + 4, y, PANEL_W - 8, mouseX, mouseY);
						y += w.height();
					}
					y += 2;
				}
			}
		}
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		for (int i = panels.size() - 1; i >= 0; i--) {
			Panel p = panels.get(i);
			if (!Theme.inside(mx, my, p.x, p.y, PANEL_W, panelHeight(p))) continue;

			if (my < p.y + HEADER_H) {
				if (button == 0) {
					dragging = p;
					dragOffX = (int) mx - p.x;
					dragOffY = (int) my - p.y;
				}
				return true;
			}
			int y = p.y + HEADER_H;
			for (Module m : p.modules) {
				if (my >= y && my < y + ROW_H) {
					if (button == 0) m.toggle();
					else if (button == 1 && !m.settings().isEmpty() && !expanded.remove(m)) expanded.add(m);
					return true;
				}
				y += ROW_H;
				if (expanded.contains(m)) {
					for (SettingWidget w : widgetsOf(m)) {
						if (w.mouseClicked(mx, my, button)) {
							activeWidget = w;
							return true;
						}
						y += w.height();
					}
					y += 2;
				}
			}
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button, double dx, double dy) {
		if (dragging != null) {
			int[] pos = PanelMath.clamp((int) mx - dragOffX, (int) my - dragOffY, PANEL_W, HEADER_H, screenW, screenH);
			dragging.x = pos[0];
			dragging.y = pos[1];
			return true;
		}
		return activeWidget != null && activeWidget.mouseDragged(mx, my);
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (dragging != null) {
			FrogClient.gui.panelPositions().put(dragging.category, new int[]{dragging.x, dragging.y});
			dragging = null;
			return true;
		}
		if (activeWidget != null) {
			activeWidget.mouseReleased();
			activeWidget = null;
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double scrollY) {
		return false;
	}
}

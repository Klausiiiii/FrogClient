package dev.frogclient.gui.widget;

import net.minecraft.client.gui.GuiGraphicsExtractor;

/**
 * Renders and edits one setting. Both layouts use the same widgets; they only
 * decide where a widget goes. Bounds are remembered from the last render.
 */
public abstract class SettingWidget {
	protected int x;
	protected int y;
	protected int width;

	public abstract int height();

	public final void render(GuiGraphicsExtractor g, int x, int y, int width, int mouseX, int mouseY) {
		this.x = x;
		this.y = y;
		this.width = width;
		draw(g, mouseX, mouseY);
	}

	protected abstract void draw(GuiGraphicsExtractor g, int mouseX, int mouseY);

	protected boolean hovered(double mx, double my) {
		return mx >= x && mx < x + width && my >= y && my < y + height();
	}

	public boolean mouseClicked(double mx, double my, int button) {
		if (button != 0 || !hovered(mx, my)) return false;
		onClick(mx, my);
		return true;
	}

	protected abstract void onClick(double mx, double my);

	public boolean mouseDragged(double mx, double my) {
		return false;
	}

	public void mouseReleased() {
	}
}

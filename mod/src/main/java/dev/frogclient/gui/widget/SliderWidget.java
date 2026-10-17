package dev.frogclient.gui.widget;

import dev.frogclient.gui.Theme;
import dev.frogclient.setting.SliderSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

public final class SliderWidget extends SettingWidget {
	private static final int BAR_H = 4;

	private final SliderSetting setting;
	private boolean dragging;
	private double shownValue = Double.NaN;
	private String shownText = "";

	public SliderWidget(SliderSetting setting) {
		this.setting = setting;
	}

	@Override
	public int height() {
		return 22;
	}

	@Override
	protected void draw(GuiGraphicsExtractor g, int mouseX, int mouseY) {
		var font = Minecraft.getInstance().font;
		g.text(font, Component.translatable(setting.nameKey()), x + 2, y + 2, Theme.TEXT, false);
		String value = valueText();
		g.text(font, value, x + width - 2 - font.width(value), y + 2, Theme.ACCENT, false);

		int barX = x + 2;
		int barW = width - 4;
		int barY = y + 14;
		double t = (setting.value() - setting.min()) / (setting.max() - setting.min());
		g.fill(barX, barY, barX + barW, barY + BAR_H, Theme.SWITCH_OFF);
		g.fill(barX, barY, barX + (int) Math.round(barW * t), barY + BAR_H, Theme.ACCENT);
	}

	private String valueText() {
		if (setting.value() != shownValue) {
			shownValue = setting.value();
			shownText = setting.step() >= 1 ? String.valueOf((long) shownValue) : String.format("%.2f", shownValue).replaceAll("0$", "");
		}
		return shownText;
	}

	@Override
	protected void onClick(double mx, double my) {
		dragging = true;
		setFromMouse(mx);
	}

	@Override
	public boolean mouseDragged(double mx, double my) {
		if (!dragging) return false;
		setFromMouse(mx);
		return true;
	}

	@Override
	public void mouseReleased() {
		dragging = false;
	}

	private void setFromMouse(double mx) {
		double t = Math.max(0, Math.min(1, (mx - (x + 2)) / (width - 4)));
		setting.set(setting.min() + t * (setting.max() - setting.min()));
	}
}

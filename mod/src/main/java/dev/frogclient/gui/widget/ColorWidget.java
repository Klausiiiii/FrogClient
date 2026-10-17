package dev.frogclient.gui.widget;

import dev.frogclient.gui.Theme;
import dev.frogclient.setting.ColorSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

public final class ColorWidget extends SettingWidget {
	private final ColorSetting setting;

	public ColorWidget(ColorSetting setting) {
		this.setting = setting;
	}

	@Override
	public int height() {
		return 14;
	}

	@Override
	protected void draw(GuiGraphicsExtractor g, int mouseX, int mouseY) {
		var font = Minecraft.getInstance().font;
		g.text(font, Component.translatable(setting.nameKey()), x + 2, y + 3, Theme.TEXT, false);
		int sx = x + width - 14;
		g.fill(sx - 1, y + 1, sx + 11, y + 13, Theme.MUTED);
		g.fill(sx, y + 2, sx + 10, y + 12, setting.argb());
	}

	@Override
	protected void onClick(double mx, double my) {
		setting.next();
	}
}

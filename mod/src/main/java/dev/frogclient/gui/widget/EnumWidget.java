package dev.frogclient.gui.widget;

import dev.frogclient.gui.Theme;
import dev.frogclient.setting.EnumSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

import java.util.Locale;

public final class EnumWidget extends SettingWidget {
	private final EnumSetting<?> setting;

	public EnumWidget(EnumSetting<?> setting) {
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
		Component value = Component.translatable(setting.nameKey() + "." + setting.get().name().toLowerCase(Locale.ROOT));
		g.text(font, value, x + width - 2 - font.width(value), y + 3, Theme.ACCENT, false);
	}

	@Override
	protected void onClick(double mx, double my) {
		setting.next();
	}
}

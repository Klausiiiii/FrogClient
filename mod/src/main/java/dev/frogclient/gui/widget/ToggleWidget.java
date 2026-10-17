package dev.frogclient.gui.widget;

import dev.frogclient.gui.Theme;
import dev.frogclient.setting.BoolSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

public final class ToggleWidget extends SettingWidget {
	private final BoolSetting setting;

	public ToggleWidget(BoolSetting setting) {
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
		Theme.drawSwitch(g, x + width - Theme.SWITCH_W - 2, y + 2, setting.isOn());
	}

	@Override
	protected void onClick(double mx, double my) {
		setting.toggle();
	}
}

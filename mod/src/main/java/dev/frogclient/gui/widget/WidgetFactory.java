package dev.frogclient.gui.widget;

import dev.frogclient.module.Module;
import dev.frogclient.setting.BoolSetting;
import dev.frogclient.setting.ColorSetting;
import dev.frogclient.setting.EnumSetting;
import dev.frogclient.setting.Setting;
import dev.frogclient.setting.SliderSetting;

import java.util.List;

public final class WidgetFactory {
	private WidgetFactory() {
	}

	public static SettingWidget create(Setting<?> setting) {
		return switch (setting) {
			case BoolSetting b -> new ToggleWidget(b);
			case SliderSetting s -> new SliderWidget(s);
			case EnumSetting<?> e -> new EnumWidget(e);
			case ColorSetting c -> new ColorWidget(c);
		};
	}

	public static List<SettingWidget> forModule(Module module) {
		return module.settings().stream().map(WidgetFactory::create).toList();
	}
}

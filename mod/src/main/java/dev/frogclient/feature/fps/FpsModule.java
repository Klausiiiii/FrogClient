package dev.frogclient.feature.fps;

import dev.frogclient.hud.HudElement;
import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import dev.frogclient.setting.BoolSetting;
import dev.frogclient.setting.ColorSetting;
import dev.frogclient.setting.SliderSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;

public final class FpsModule extends Module implements HudElement {
	private static final int PADDING = 2;

	public final SliderSetting scale = add(new SliderSetting("scale", "frogclient.setting.fps.scale", 0.5, 3, 0.25, 1));
	public final ColorSetting color = add(new ColorSetting("color", "frogclient.setting.fps.color", 0xFFFFFFFF));
	public final BoolSetting background = add(new BoolSetting("background", "frogclient.setting.fps.background", false));

	private int cachedFps = -1;
	private String cachedText = "";

	public FpsModule() {
		super("fps", Category.HUD, true);
	}

	/** Returns the display text; the same instance while the FPS value is unchanged (no per-frame allocation). */
	public String text(int fps) {
		if (fps != cachedFps) {
			cachedFps = fps;
			cachedText = fps + " FPS";
		}
		return cachedText;
	}

	@Override
	public String hudId() {
		return "fps";
	}

	@Override
	public int width() {
		var font = Minecraft.getInstance().font;
		return (int) Math.ceil((font.width(text(Minecraft.getInstance().getFps())) + 2 * PADDING) * scale.value());
	}

	@Override
	public int height() {
		return (int) Math.ceil((Minecraft.getInstance().font.lineHeight + 2 * PADDING) * scale.value());
	}

	@Override
	public boolean visible() {
		return isEnabled();
	}

	@Override
	public void render(GuiGraphicsExtractor g, int x, int y) {
		var mc = Minecraft.getInstance();
		String s = text(mc.getFps());
		float k = (float) scale.value();
		g.pose().pushMatrix();
		g.pose().translate(x, y);
		g.pose().scale(k, k);
		if (background.isOn()) {
			g.fill(0, 0, mc.font.width(s) + 2 * PADDING, mc.font.lineHeight + 2 * PADDING, 0x80000000);
		}
		g.text(mc.font, s, PADDING, PADDING, color.argb(), true);
		g.pose().popMatrix();
	}
}

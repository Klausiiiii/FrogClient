package dev.frogclient.hud;

import dev.frogclient.FrogClient;
import dev.frogclient.config.GuiState;
import dev.frogclient.module.ModuleManager;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.resources.Identifier;

import java.util.List;

public final class HudRenderer {
	private HudRenderer() {
	}

	public static void register(ModuleManager modules, GuiState gui) {
		List<HudElement> elements = modules.all().stream()
				.filter(m -> m instanceof HudElement)
				.map(m -> (HudElement) m)
				.toList();
		HudElementRegistry.addLast(Identifier.fromNamespaceAndPath(FrogClient.MOD_ID, "hud"), (g, delta) -> {
			if (Minecraft.getInstance().getDebugOverlay().showDebugScreen()) return;
			for (HudElement e : elements) {
				if (e.visible()) renderAt(g, e, gui);
			}
		});
	}

	/** Renders an element at its saved (or default) position; also used by the HUD editor. */
	public static int[] renderAt(GuiGraphicsExtractor g, HudElement e, GuiState gui) {
		int[] px = position(e, gui).toPixels(g.guiWidth(), g.guiHeight(), e.width(), e.height());
		e.render(g, px[0], px[1]);
		return px;
	}

	public static HudPosition position(HudElement e, GuiState gui) {
		double[] p = gui.hudPositions().get(e.hudId());
		return p == null ? HudPosition.DEFAULT_TOP_LEFT : new HudPosition(p[0], p[1]);
	}
}

package dev.frogclient;

import com.mojang.blaze3d.platform.InputConstants;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.KeyMapping;
import net.minecraft.resources.Identifier;

public final class FrogKeys {
	public static KeyMapping OPEN_GUI;
	public static KeyMapping ZOOM;

	private FrogKeys() {
	}

	public static void register() {
		KeyMapping.Category category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath(FrogClient.MOD_ID, "main"));
		OPEN_GUI = KeyMappingHelper.registerKeyMapping(
				new KeyMapping("key.frogclient.open_gui", InputConstants.Type.KEYBOARD, InputConstants.KEY_RSHIFT, category));
		ZOOM = KeyMappingHelper.registerKeyMapping(
				new KeyMapping("key.frogclient.zoom", InputConstants.Type.KEYBOARD, InputConstants.KEY_C, category));
	}
}

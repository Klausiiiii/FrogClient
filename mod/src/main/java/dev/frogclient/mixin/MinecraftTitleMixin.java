package dev.frogclient.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import dev.frogclient.WindowTitle;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(Minecraft.class)
public abstract class MinecraftTitleMixin {
	@ModifyReturnValue(method = "createTitle", at = @At("RETURN"))
	private String frogclient$instanceTitle(String title) {
		return WindowTitle.format(title, System.getProperty(WindowTitle.PROPERTY));
	}
}

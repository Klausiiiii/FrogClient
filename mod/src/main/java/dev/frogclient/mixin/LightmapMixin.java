package dev.frogclient.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import dev.frogclient.FrogClient;
import dev.frogclient.feature.FullbrightModule;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(LightmapRenderStateExtractor.class)
public abstract class LightmapMixin {
	/** The first Double#floatValue in extract() reads the gamma option. */
	@ModifyExpressionValue(
			method = "extract",
			at = @At(value = "INVOKE", target = "Ljava/lang/Double;floatValue()F", ordinal = 0))
	private float frogclient$fullbright(float gamma) {
		return (float) FrogClient.modules.get(FullbrightModule.class).effectiveGamma(gamma);
	}
}

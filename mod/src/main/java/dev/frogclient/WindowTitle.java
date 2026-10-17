package dev.frogclient;

/** Window title per launcher instance, so parallel instances can be told apart. */
public final class WindowTitle {
	public static final String PROPERTY = "frogclient.instanceName";

	private WindowTitle() {
	}

	public static String format(String vanillaTitle, String instanceName) {
		if (instanceName == null || instanceName.isBlank()) return vanillaTitle;
		return "FrogClient – " + instanceName.strip();
	}
}

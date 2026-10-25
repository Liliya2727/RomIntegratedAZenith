package vendor.azenith;

interface IAZenithService {
    const int VERSION = 1;

    int getApiVersion();
    boolean setServiceEnabled(boolean enabled);
    boolean setConfig(in String key, in String value);
    boolean addGame(in String pkg);
    boolean removeGame(in String pkg);
    String getGames();
    String getState();
    void applyNow();
    boolean isServiceEnabled();
}

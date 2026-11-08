package vendor.azenith;

interface IAZenithService {
    const int VERSION = 3;

    int getApiVersion();
    boolean setDisabled(boolean disabled);
    boolean isDisabled();
    boolean setConfig(in String key, in String value);
    boolean addGame(in String pkg);
    boolean removeGame(in String pkg);
    String getGames();
    String getState();
}

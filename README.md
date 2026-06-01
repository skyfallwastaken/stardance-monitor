# stardance Monitor

_From the creators of Flavortown Tracker & [SOM Monitor](https://go.skyfall.dev/som-monitor)_

Tracks the stardance shop for price updates and new items.

## Setup

Clone the repo:

```bash
git clone https://github.com/skyfallwastaken/stardance-tracker
cd stardance-tracker
```

Configure the `.env`:

```env
COOKIE= # stardance.hackclub.com cookie
WEBHOOK_URL= # slack webhook url
USER_AGENT= # optional
BASE_URL= # optional - defaults to stardance's prod instance
STORAGE_PATH= # optional - defaults to `stardance-storage` folder in working dir
```

Then run:

```bash
chmod +x ./scripts/run-every-5min.sh
./scripts/run-every-5min.sh
```

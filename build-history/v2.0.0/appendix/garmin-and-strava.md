# Garmin and Strava

## Garmin personal connection

The token request is the one in Garmin's personal API instructions:
`POST /personalapitoken-service/personal-api/personal-api-key/token` with
header `Authorization: ApiKey <key>`. The response body is a JWT. Activities
are read from
`GET /activitylist-service/activities/search/activities`. One day of heart-rate
variability is read from `GET /hrv-service/hrv/<date>`.

These paths are the ones published in Garmin's
[personal API page](https://developer.garmin.com/gc-developer-program/personal/personal-api/)
and the
[example client](https://github.com/garmin/connect-personal-api-python).
Tests call a local server, not Garmin.

## Strava permissions

This importer reads a file the person already downloaded. It does not call
Strava's API. These public pages describe the download and the API rules, and
they do not settle whether a local tool may keep the download:

- [Strava API Policy, bulk export](https://cdn-1a.strava.com/legal/api_policy)
- [Strava Privacy Policy](https://www.strava.com/legal/privacy)
- [Strava Terms](https://www.strava.com/legal/terms)
- [Strava API Agreement](https://www.strava.com/legal/api)

That question stays open.

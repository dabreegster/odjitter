
# odjitter

This repo contains the `odjitter` crate that implements a ‘jittering’
technique for pre-processing origin-destination (OD) data and an
associated R interface package (see the [r](r/) subdirectory). We hope
to support other languages in the future (see [issue
\#23](https://github.com/dabreegster/odjitter/issues/23)).

## What is jittering?

Jittering is a method that takes OD data in a .csv file plus zones and
geographic datasets representing trip start and end points in .geojson
files and outputs geographic lines representing movement between the
zones that can be stored as GeoJSON files. The name comes from jittering
in a [data visualisation
context](https://ggplot2-book.org/layers.html?q=noise#position), which
refers to the addition of random noise to the location of points,
preventing them overlapping.

## Why jitter?

For a more detailed description of the method and an explanation of why
it is useful, especially when modeling active modes that require dense
active travel networks, see the paper [Jittering: A Computationally
Efficient Method for Generating Realistic Route Networks from
Origin-Destination
Data](https://findingspress.org/article/33873-jittering-a-computationally-efficient-method-for-generating-realistic-route-networks-from-origin-destination-data)
(Lovelace, Félix, and Carlino 2022).

# Installation

Install the package from the system command line as follows (you need to
have installed and set-up
[cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
first):

``` bash
cargo install --git https://github.com/dabreegster/odjitter
```

To check the package installation worked, you can run `odjitter` command
without arguments. If it prints the following message congratulations,
it works 🎉

``` bash
odjitter
```

    odjitter 0.1.0
    Dustin Carlino <dabreegster@gmail.com
    Disaggregate origin/destination data from zones to points

    USAGE:
        odjitter <SUBCOMMAND>

    OPTIONS:
        -h, --help       Print help information
        -V, --version    Print version information

    SUBCOMMANDS:
        disaggregate    Fully disaggregate input desire lines into output representing one trip
                            each, with a `mode` column
        help            Print this message or the help of the given subcommand(s)
        jitter          Import raw data and build an activity model for a region

As shown in the output above the `odjitter` command line tools has
subcommands: `disaggregate` and `jitter`. The main difference between
these commands is that `jitter` returns OD pairs representing multiple
trips or fractions of a trip. `disaggregate`, by contrast, returns data
representing single trips.

## Docker

Alternatively, you can run through Docker: `docker run -t abstreet/odjitter <CMD>`. See below for command line usage, or start with `help`.

(For maintainers only: to build and push a new version, `docker build -t odjitter . && docker tag odjitter abstreet/odjitter:latest && docker push abstreet/odjitter:latest`.)

# `jitter` OD data

To jitter OD data you need a minimum of three inputs, examples of which
are provided in the [`data/`
folder](https://github.com/dabreegster/odjitter/tree/main/data) of this
repo, the first few lines of which are illustrated below:

1.  A [.csv
    file](https://github.com/dabreegster/odjitter/blob/main/data/od.csv)
    containing OD data with two columns containing zone IDs (specified
    with `--origin-key=geo_code1 --destination-key=geo_code2` by
    default) and other columns representing trip counts:

| geo_code1 | geo_code2 | all | from_home | train | bus | car_driver | car_passenger | bicycle | foot | other |
|:----------|:----------|----:|----------:|------:|----:|-----------:|--------------:|--------:|-----:|------:|
| S02001616 | S02001616 |  82 |         0 |     0 |   3 |          6 |             0 |       2 |   71 |     0 |
| S02001616 | S02001620 | 188 |         0 |     0 |  42 |         26 |             3 |      11 |  105 |     1 |
| S02001616 | S02001621 |  99 |         0 |     0 |  13 |          7 |             3 |      15 |   61 |     0 |

2.  A [.geojson
    file](https://github.com/dabreegster/odjitter/blob/main/data/zones.geojson)
    representing zones that contains values matching the zone IDs in the
    OD data (the field containing zone IDs is specified with
    `--zone-name-key=InterZone` by default):

``` bash
head -6 data/zones.geojson
```

    {
    "type": "FeatureCollection",
    "name": "zones_min",
    "crs": { "type": "name", "properties": { "name": "urn:ogc:def:crs:OGC:1.3:CRS84" } },
    "features": [
    { "type": "Feature", "properties": { "InterZone": "S02001616", "Name": "Merchiston and Greenhill", "TotPop2011": 5018, "ResPop2011": 4730, "HHCnt2011": 2186, "StdAreaHa": 126.910911, "StdAreaKm2": 1.269109, "Shape_Leng": 9073.5402482000009, "Shape_Area": 1269109.10155 }, "geometry": { "type": "MultiPolygon", "coordinates": [ [ [ [ -3.2040366, 55.9333372 ], [ -3.2036354, 55.9321624 ], [ -3.2024036, 55.9321874 ], [ -3.2019838, 55.9315586 ], [ -3.2005071, 55.9317411 ], [ -3.199902, 55.931113 ], [ -3.2033504, 55.9308279 ], [ -3.2056319, 55.9309507 ], [ -3.2094979, 55.9308666 ], [ -3.2109753, 55.9299985 ], [ -3.2107073, 55.9285904 ], [ -3.2124928, 55.927854 ], [ -3.2125633, 55.9264661 ], [ -3.2094928, 55.9265616 ], [ -3.212929, 55.9260741 ], [ -3.2130774, 55.9264384 ], [ -3.2183973, 55.9252709 ], [ -3.2208941, 55.925282 ], [ -3.2242732, 55.9258683 ], [ -3.2279975, 55.9277452 ], [ -3.2269867, 55.928489 ], [ -3.2267625, 55.9299817 ], [ -3.2254561, 55.9307854 ], [ -3.224148, 55.9300725 ], [ -3.2197791, 55.9315472 ], [ -3.2222706, 55.9339127 ], [ -3.2224909, 55.934809 ], [ -3.2197844, 55.9354692 ], [ -3.2204535, 55.936195 ], [ -3.218362, 55.9368806 ], [ -3.2165749, 55.937069 ], [ -3.215582, 55.9380761 ], [ -3.2124132, 55.9355465 ], [ -3.212774, 55.9347972 ], [ -3.2119068, 55.9341947 ], [ -3.210138, 55.9349668 ], [ -3.208051, 55.9347716 ], [ -3.2083105, 55.9364224 ], [ -3.2053546, 55.9381495 ], [ -3.2046077, 55.9395298 ], [ -3.20356, 55.9380951 ], [ -3.2024323, 55.936318 ], [ -3.2029121, 55.935831 ], [ -3.204832, 55.9357555 ], [ -3.2040366, 55.9333372 ] ] ] ] } },

3.  One or more [.geojson
    file](https://github.com/dabreegster/odjitter/blob/main/data/road_network.geojson)
    representing geographic entities (e.g. road networks) from which
    origin and destination points are sampled

``` bash
head -6 data/road_network.geojson
```

    {
    "type": "FeatureCollection",
    "name": "road_network_min",
    "crs": { "type": "name", "properties": { "name": "urn:ogc:def:crs:OGC:1.3:CRS84" } },
    "features": [
    { "type": "Feature", "properties": { "osm_id": "3468", "name": "Albyn Place", "highway": "tertiary", "waterway": null, "aerialway": null, "barrier": null, "man_made": null, "access": null, "bicycle": null, "service": null, "z_order": 4, "other_tags": "\"lit\"=>\"yes\",\"lanes\"=>\"3\",\"maxspeed\"=>\"20 mph\",\"sidewalk\"=>\"both\",\"lanes:forward\"=>\"2\",\"lanes:backward\"=>\"1\"" }, "geometry": { "type": "LineString", "coordinates": [ [ -3.207438, 55.9533584 ], [ -3.2065953, 55.9535098 ] ] } },

The `jitter` command requires you to set the maximum number of trips for
all trips in the jittered result, with the argument
\`disaggregation-threshold\`\`. A value of 1 will create a line for
every trip in the dataset, a value above the maximum number of trips in
the ‘all’ column in the OD data will result in a jittered dataset that
has the same number of desire lines (the geographic representation of OD
pairs) as in the input (50 in this case).

With reference to the test data in this repo, you can run the `jitter`
command line tool as follows:

``` bash
odjitter jitter --od-csv-path data/od.csv \
  --zones-path data/zones.geojson \
  --subpoints-origins-path data/road_network.geojson \
  --subpoints-destinations-path data/road_network.geojson \
  --disaggregation-threshold 50 \
  --output-path data/output_max50.geojson
```

    Scraped 7 zones from data/zones.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Disaggregating OD data
    Wrote data/output_max50.geojson

Try running it with a different `disaggregation-threshold` value (10 in
the command below):

``` bash
odjitter jitter --od-csv-path data/od.csv \
  --zones-path data/zones.geojson \
  --subpoints-origins-path data/road_network.geojson \
  --subpoints-destinations-path data/road_network.geojson \
  --disaggregation-threshold 10 \
  --output-path data/output_max10.geojson
```

    Scraped 7 zones from data/zones.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Disaggregating OD data
    Wrote data/output_max10.geojson

You can run odjitter on OD datasets in which the features in the origins
are different from the features in the destinations, e.g. if you have
data on movement between residential areas and parks. However, you need
to first combine the geographic dataset representing origins and the
geographic destinations representing destinations into a single object.
An example of this type of this is is demonstrated in the code chunk
below.

``` bash
odjitter jitter --od-csv-path data/od_destinations.csv \
  --zones-path data/zones_combined.geojson \
  --subpoints-origins-path data/road_network.geojson \
  --subpoints-destinations-path data/road_network.geojson \
  --disaggregation-threshold 50 \
  --output-path data/output_destinations_differ_50.geojson
```

    Scraped 9 zones from data/zones_combined.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Disaggregating OD data
    Wrote data/output_destinations_differ_50.geojson

# Outputs

The figure below shows the output of the `jitter` commands above
visually, with the left image showing unjittered results with origins
and destinations going to zone centroids (as in many if not most
visualisations of desire lines between zones), the central image showing
the result after setting `disaggregation-threshold` argument to 50, and
the right hand figure showing the result after setting
`disaggregation-threshold` to 10.

You can call the Rust code from R, as illustrated by the code below
which generates the datasets shown in the figures below.

``` r
remotes::install_github("dabreegster/odjitter", subdir = "r")
# Note: code to generate the visualisation below
od = readr::read_csv("data/od.csv")
zones = sf::read_sf("data/zones.geojson")
network = sf::read_sf("data/road_network.geojson")
od_sf = od::od_to_sf(od, zones)
odjittered_max_50 = odjitter::jitter(od, zones, network, disaggregation_threshold = 50)
odjittered_max_10 = odjitter::jitter(od, zones, network, disaggregation_threshold = 10)
```

![Demonstration of the effect of the disaggregation threshold on the
number of desire
lines](README_files/figure-commonmark/thresholddemo-1.png)

Note: `odjitter` uses a random number generator to sample points, so the
output will change each time you run it, unless you set the `rng-seed`,
as documented in the next section.

The `subpoints-origins-path` and `subpoints-destinations-path` can be
used to generate jittered desire lines that start from or go to
particular points, defined in .geojson files. We will demonstrate this
on a simple imaginary example:

``` bash
head data/od_schools.csv
```

    origin,destination,walk,bike,other,car
    S02001616,S02001616,232,8,70,0
    S02001620,S02001616,87,3,26,223
    S02001621,S02001616,80,3,24,250
    S02001622,S02001616,64,2,19,348
    S02001623,S02001616,52,2,15,464
    S02001656,S02001616,62,2,19,366
    S02001660,S02001616,77,3,23,266
    S02001616,S02001620,7,0,2,17
    S02001620,S02001620,18,1,5,0

Set the origin, destination, and threshold keys (to car meaning that the
max n. car trips per OD pair is 10 in this case) as follows:

``` bash
odjitter jitter --od-csv-path data/od_schools.csv \
  --zones-path data/zones.geojson \
  --origin-key origin \
  --destination-key destination \
  --subpoints-origins-path data/road_network.geojson \
  --subpoints-destinations-path data/schools.geojson \
  --disaggregation-key car \
  --disaggregation-threshold 10 \
  --output-path output_max10_schools.geojson
```

    Scraped 7 zones from data/zones.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Scraped 31 subpoints from data/schools.geojson
    Disaggregating OD data
    Wrote output_max10_schools.geojson

You can also set weights associated with each origin and destination in
the input data. The following example weights trips to schools
proportional to the values in the ‘weight’ key for each imaginary data
point represented in the `schools.geojson` object:

``` bash
odjitter jitter --od-csv-path data/od_schools.csv \
  --zones-path data/zones.geojson \
  --origin-key origin \
  --destination-key destination \
  --subpoints-origins-path data/road_network.geojson \
  --subpoints-destinations-path data/schools.geojson \
  --disaggregation-key car \
  --disaggregation-threshold 10 \
  --weight-key-destinations weight \
  --output-path output_max10_schools_with_weights.geojson
```

    Scraped 7 zones from data/zones.geojson
    Scraped 5073 subpoints from data/road_network.geojson
    Scraped 31 subpoints from data/schools.geojson
    Disaggregating OD data
    Wrote output_max10_schools_with_weights.geojson

# `disaggregate` OD data

Sometimes it’s useful to convert aggregate OD datasets into movement
data at the trip level, with one record per trip or stage.
Microsumulation or agent-based modelling in transport simulation
software such as [A/B Street](https://github.com/a-b-street/abstreet) is
an example where disaggregate data may be needed. The `disaggregate`
command does this full disaggregation work, as demonstrated below.

``` bash
odjitter disaggregate --od-csv-path data/od.csv \
  --zones-path data/zones.geojson \
  --output-path output_individual.geojson
```

    Scraped 7 zones from data/zones.geojson
    Disaggregating OD data
    Wrote output_individual.geojson

``` bash
head output_individual.geojson
rm output_individual.geojson
```

    {"type":"FeatureCollection", "features":[
    {"geometry":{"coordinates":[[-3.2263977926488985,55.92783397974489],[-3.2097949190090564,55.931894382403456]],"type":"LineString"},"properties":{"mode":"bus"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.214452310499139,55.926362026835776],[-3.2099011140196207,55.93444441681924]],"type":"LineString"},"properties":{"mode":"bus"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.2135400698085235,55.93035182421444],[-3.2244453899330097,55.9290303580713]],"type":"LineString"},"properties":{"mode":"bus"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.226151458277428,55.92976711232548],[-3.213318278973036,55.93497592560621]],"type":"LineString"},"properties":{"mode":"bicycle"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.2185705494212358,55.926226845455034],[-3.2115019430114167,55.93197469392582]],"type":"LineString"},"properties":{"mode":"bicycle"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.2064406813843465,55.93266248325375],[-3.2190062133419635,55.92613966571992]],"type":"LineString"},"properties":{"mode":"car_driver"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.226009373785913,55.9285088488262],[-3.2149550551179176,55.93495382922043]],"type":"LineString"},"properties":{"mode":"car_driver"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.2152401192504443,55.932554427847144],[-3.214478335328521,55.933957525733355]],"type":"LineString"},"properties":{"mode":"car_driver"},"type":"Feature"},
    {"geometry":{"coordinates":[[-3.218021802161658,55.92963564155289],[-3.22510485680737,55.92984949438051]],"type":"LineString"},"properties":{"mode":"car_driver"},"type":"Feature"},

# Details

For full details on the arguments of each of `odjitter`’s subcommands
can be viewed with the `--help` flag:

``` bash
odjitter jitter --help
odjitter disaggregate --help
```

    odjitter-jitter 
    Import raw data and build an activity model for a region

    USAGE:
        odjitter jitter [OPTIONS] --od-csv-path <OD_CSV_PATH> --zones-path <ZONES_PATH> --output-path <OUTPUT_PATH> --disaggregation-threshold <DISAGGREGATION_THRESHOLD>

    OPTIONS:
            --deduplicate-pairs
                Prevent duplicate (origin, destination) pairs from appearing in the output. This may
                increase memory and runtime requirements. Note the duplication uses the floating point
                precision of the input data, and only consider geometry (not any properties)

            --destination-key <DESTINATION_KEY>
                Which column in the OD row specifies the zone where trips ends? [default: geo_code2]

            --disaggregation-key <DISAGGREGATION_KEY>
                Which column in the OD row specifies the total number of trips to disaggregate?
                [default: all]

            --disaggregation-threshold <DISAGGREGATION_THRESHOLD>
                What's the maximum number of trips per output OD row that's allowed? If an input OD row
                contains less than this, it will appear in the output without transformation. Otherwise,
                the input row is repeated until the sum matches the original value, but each output row
                obeys this maximum

        -h, --help
                Print help information

            --min-distance-meters <MIN_DISTANCE_METERS>
                Guarantee that jittered origin and destination points are at least this distance apart
                [default: 1.0]

            --od-csv-path <OD_CSV_PATH>
                The path to a CSV file with aggregated origin/destination data

            --origin-key <ORIGIN_KEY>
                Which column in the OD row specifies the zone where trips originate? [default:
                geo_code1]

            --output-path <OUTPUT_PATH>
                The path to a GeoJSON file where the output will be written

            --rng-seed <RNG_SEED>
                By default, the output will be different every time the tool is run, based on a
                different random number generator seed. Specify this to get deterministic behavior,
                given the same input

            --subpoints-destinations-path <SUBPOINTS_DESTINATIONS_PATH>
                The path to a GeoJSON file to use for sampling subpoints for destination zones. If this
                isn't specified, random points within each zone will be used instead

            --subpoints-origins-path <SUBPOINTS_ORIGINS_PATH>
                The path to a GeoJSON file to use for sampling subpoints for origin zones. If this isn't
                specified, random points within each zone will be used instead

            --weight-key-destinations <WEIGHT_KEY_DESTINATIONS>
                If specified, this column will be used to more frequently choose subpoints in
                `subpoints_destinations_path` with a higher weight value. Otherwise all subpoints will
                be equally likely to be chosen

            --weight-key-origins <WEIGHT_KEY_ORIGINS>
                If specified, this column will be used to more frequently choose subpoints in
                `subpoints_origins_path` with a higher weight value. Otherwise all subpoints will be
                equally likely to be chosen

            --zone-name-key <ZONE_NAME_KEY>
                In the zones GeoJSON file, which property is the name of a zone [default: InterZone]

            --zones-path <ZONES_PATH>
                The path to a GeoJSON file with named zones
    odjitter-disaggregate 
    Fully disaggregate input desire lines into output representing one trip each, with a `mode` column

    USAGE:
        odjitter disaggregate [OPTIONS] --od-csv-path <OD_CSV_PATH> --zones-path <ZONES_PATH> --output-path <OUTPUT_PATH>

    OPTIONS:
            --deduplicate-pairs
                Prevent duplicate (origin, destination) pairs from appearing in the output. This may
                increase memory and runtime requirements. Note the duplication uses the floating point
                precision of the input data, and only consider geometry (not any properties)

            --destination-key <DESTINATION_KEY>
                Which column in the OD row specifies the zone where trips ends? [default: geo_code2]

        -h, --help
                Print help information

            --min-distance-meters <MIN_DISTANCE_METERS>
                Guarantee that jittered origin and destination points are at least this distance apart
                [default: 1.0]

            --od-csv-path <OD_CSV_PATH>
                The path to a CSV file with aggregated origin/destination data

            --origin-key <ORIGIN_KEY>
                Which column in the OD row specifies the zone where trips originate? [default:
                geo_code1]

            --output-path <OUTPUT_PATH>
                The path to a GeoJSON file where the output will be written

            --rng-seed <RNG_SEED>
                By default, the output will be different every time the tool is run, based on a
                different random number generator seed. Specify this to get deterministic behavior,
                given the same input

            --subpoints-destinations-path <SUBPOINTS_DESTINATIONS_PATH>
                The path to a GeoJSON file to use for sampling subpoints for destination zones. If this
                isn't specified, random points within each zone will be used instead

            --subpoints-origins-path <SUBPOINTS_ORIGINS_PATH>
                The path to a GeoJSON file to use for sampling subpoints for origin zones. If this isn't
                specified, random points within each zone will be used instead

            --weight-key-destinations <WEIGHT_KEY_DESTINATIONS>
                If specified, this column will be used to more frequently choose subpoints in
                `subpoints_destinations_path` with a higher weight value. Otherwise all subpoints will
                be equally likely to be chosen

            --weight-key-origins <WEIGHT_KEY_ORIGINS>
                If specified, this column will be used to more frequently choose subpoints in
                `subpoints_origins_path` with a higher weight value. Otherwise all subpoints will be
                equally likely to be chosen

            --zone-name-key <ZONE_NAME_KEY>
                In the zones GeoJSON file, which property is the name of a zone [default: InterZone]

            --zones-path <ZONES_PATH>
                The path to a GeoJSON file with named zones

# Similar work

The technique is implemented in the function
[`od_jitter()`](https://itsleeds.github.io/od/reference/od_jitter.html)
from the R package [`od`](https://itsleeds.github.io/od/index.html). The
functionality contained in this repo is an extended and much faster
implementation: according to our benchmarks on a large dataset it was
around 1000 times faster than the R implementation.

# References

<div id="refs" class="references csl-bib-body hanging-indent">

<div id="ref-lovelace_jittering_2022b" class="csl-entry">

Lovelace, Robin, Rosa Félix, and Dustin Carlino. 2022. “Jittering: A
Computationally Efficient Method for Generating Realistic Route Networks
from Origin-Destination Data.” *Findings*, April, 33873.
<https://doi.org/10.32866/001c.33873>.

</div>

</div>

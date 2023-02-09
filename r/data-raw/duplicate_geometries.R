## code to prepare `duplicate_geometries` dataset goes here

remotes::install_github("dabreegster/odjitter", subdir = "r")

library(odjitter)
# ?odjitter::jitter

# usethis::use_data(duplicate_geometries, overwrite = TRUE)
od = readr::read_csv("https://github.com/dabreegster/odjitter/raw/main/data/od.csv")
zones = sf::read_sf("https://github.com/dabreegster/odjitter/raw/main/data/zones.geojson")
road_network = sf::read_sf("https://github.com/dabreegster/odjitter/raw/main/data/road_network.geojson")
od_jittered = jitter(od, zones, subpoints = road_network)
# basic example to test it's a hard-to-find issue:
summary(duplicated(od_jittered$geometry))

# Larger example:
od_jittered = jitter(
  od,
  zones,
  subpoints = road_network,
  disaggregation_threshold = 1,
  show_command = TRUE
)
summary(duplicated(od_jittered$geometry))


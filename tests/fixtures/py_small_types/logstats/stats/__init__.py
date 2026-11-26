from logstats.stats.rate import Rate
from logstats.stats.tally import Tally
from logstats.stats.window import Window


def percent(part: int, whole: int) -> int:
    return part * 100 // whole if whole else 0

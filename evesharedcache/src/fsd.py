import sys
import importlib
import json
from json import JSONEncoder

if len(sys.argv) < 4:
	print("Missing arguments!")
	exit()

def map_to_dict(o, attributes):
	d = dict()
	for attribute in attributes:
		d[attribute] = getattr(o, attribute)
	return d

class FSDEncoder(JSONEncoder):
	def default(self, o):
		if str(type(o)) == "<type 'cfsd.dict'>":
			pdict = {}
			for key, value in o.iteritems():
				pdict[self.encode(key)] = value
			return pdict
		elif str(type(o)) == "<type 'cfsd.list'>":
			return [i for i in o]
		else:
			return map_to_dict(o, [field for field in dir(o) if not field.startswith("__")])


loader = importlib.import_module(sys.argv[1])
cdict = loader.load(sys.argv[2])
with open(sys.argv[3], "w") as file:
	json.dump(cdict, file, cls=FSDEncoder)
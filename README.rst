MensaBot
########

#########################################
A Telegram Mensa Bot for Dresden, Germany
#########################################

:Author:    Schnusch
:Date:      2018-01-17
:Copyright: AGPLv3

Commands
========

**/mensa** [<name>]
	Answer with today's or tomorrow's canteen menu, if *name* is omitted the
	menus for *general.mensas* are shown otherwise for the canteens matching
	*name* the closest.

**/about**
	Show general information about the bot.

Configuration
=============

**general**
	**token**
		Your Telegram Bot Token obtained from *@BotFather*

	**tomorrow**
		From this time onwards the menu of the next day is sent, *24:00:00* will
		disable this.

	**retries**
		Number of times every Telegram Bot API call is tried before giving up
		on the call, 0 causes an unlimited number of retries.

	**retrywait**
		Number of seconds to wait before retrying an unsuccessful call.

	**mensas**
		Array of default canteens to fall back to if no search is given, names
		must match exactly.

	**patterns**
		Array of regular expressions that trigger the same behaviour as */mensa*
		if a text messages matches any of them.

**allow**, **deny**
	**chats**
		Array of integer chat IDs

	**users**
		Array of strings that are tried to be interpreted as integer user IDs
		or if unable to do so as usernames, names can be prefixed by an @-sign
		to prevent interpretation as an ID.

Authorization
=============

Messages are processed if following rules return *True*:

.. code:: python

	if sender in allow.users:
		return True
	elif sender in deny.users:
		return False
	elif chat in allow.chats:
		return True
	elif chat in deny.chats:
		return False
	elif allow.users.is_empty() and allow.chats.is_empty():
		return True
	else:
		return False

Applying these rules yields following table:

+---------------+---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|                               |           *deny.users* empty          |                                       |         sender in *deny.users*        |
+     **message processed?**    +--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|                               | *deny.chats* |         | chat in      | *deny.chats* |         | chat in      | *deny.chats* |         | chat in      |
|                               | empty        |         | *deny.chats* | empty        |         | *deny.chats* | empty        |         | *deny.chats* |
+---------------+---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | *allow.chats* |    **yes**   | **yes** |      no      |    **yes**   | **yes** |      no      |      no      |   no    |      no      |
|               | empty         |              |         |              |              |         |              |              |         |              |
+ *allow.users* +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
| empty         |               |      no      |   no    |      no      |      no      |   no    |      no      |      no      |   no    |      no      |
+               +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | chat in       |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |      no      |   no    |      no      |
|               | *allow.chats* |              |         |              |              |         |              |              |         |              |
+---------------+---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | *allow.chats* |      no      |   no    |      no      |      no      |   no    |      no      |      no      |   no    |      no      |
|               | empty         |              |         |              |              |         |              |              |         |              |
+               +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               |               |      no      |   no    |      no      |      no      |   no    |      no      |      no      |   no    |      no      |
+               +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | chat in       |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |      no      |   no    |      no      |
|               | *allow.chats* |              |         |              |              |         |              |              |         |              |
+---------------+---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | *allow.chats* |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |
|               | empty         |              |         |              |              |         |              |              |         |              |
+ sender in     +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
| *allow.users* |               |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |
+               +---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+
|               | chat in       |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |    **yes**   | **yes** |    **yes**   |
|               | *allow.chats* |              |         |              |              |         |              |              |         |              |
+---------------+---------------+--------------+---------+--------------+--------------+---------+--------------+--------------+---------+--------------+

# Changes


## 0.6.0 (2019-07-08)

* actix-web 1.0.0 compatibility

* Upgraded logic that evaluates session state, including new SessionStatus field,
  and introduced ``session.renew()`` and ``session.purge()`` functionality.
  Use ``renew()`` to cycle the session key at successful login.  ``renew()`` keeps a
  session's state while replacing the old cookie and session key with new ones.
  Use ``purge()`` at logout to invalidate the session cookie and remove the
  session's redis cache entry.



## 0.5.1 (2018-08-02)

* Use cookie 0.11


## 0.5.0 (2018-07-21)

* Session cookie configuration

* Actix/Actix-web 0.7 compatibility


## 0.4.0 (2018-05-08)

* Actix web 0.6 compatibility

## 0.3.0 (2018-04-10)

* Actix web 0.5 compatibility

## 0.2.0 (2018-02-28)

* Use resolver actor from actix

* Use actix web 0.5

## 0.1.0 (2018-01-23)

* First release

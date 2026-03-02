interface Repository<T> {
    find(id: number): T | undefined;
    save(entity: T): boolean;
    delete(id: number): boolean;
}

type AsyncResult<T> = Promise<T | Error>;

abstract class BaseRepository<T> implements Repository<T> {
    protected items: Map<number, T> = new Map();

    abstract find(id: number): T | undefined;
    
    save(entity: T & { id: number }): boolean {
        this.items.set(entity.id, entity);
        return true;
    }

    delete(id: number): boolean {
        return this.items.delete(id);
    }
}

interface User {
    id: number;
    name: string;
    email: string;
}

class UserRepository extends BaseRepository<User> {
    find(id: number): User | undefined {
        return this.items.get(id);
    }

    async findByEmail(email: string): AsyncResult<User> {
        for (const user of this.items.values()) {
            if (user.email === email) {
                return user;
            }
        }
        return new Error('User not found');
    }
}

type Partial<T> = {
    [P in keyof T]?: T[P];
};

type Readonly<T> = {
    readonly [P in keyof T]: T[P];
};

function updateUser(user: User, updates: Partial<User>): User {
    return { ...user, ...updates };
}
